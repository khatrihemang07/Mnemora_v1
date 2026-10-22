//! JSON messages framed with a 4-byte big-endian length prefix. A frame over [`MAX_FRAME`] bytes
//! is refused: when sending, before anything is written; when receiving, from the header alone,
//! before any of the body is read, and the caller closes the connection.

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::NetError;

/// The largest frame body, in bytes (8 MB).
pub const MAX_FRAME: usize = 8 * 1024 * 1024;

/// Writes one framed message.
pub async fn write_frame<W, T>(writer: &mut W, message: &T) -> Result<(), NetError>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let body = serde_json::to_vec(message).map_err(|e| NetError::Malformed(e.to_string()))?;
    if body.len() > MAX_FRAME {
        return Err(NetError::TooLarge(body.len()));
    }
    let len = u32::try_from(body.len()).map_err(|_| NetError::TooLarge(body.len()))?;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(&body).await?;
    writer.flush().await?;
    Ok(())
}

/// Reads one framed message. A clean close between frames is [`NetError::Closed`].
pub async fn read_frame<R, T>(reader: &mut R) -> Result<T, NetError>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let mut header = [0u8; 4];
    match reader.read_exact(&mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Err(NetError::Closed),
        Err(e) => return Err(e.into()),
    }
    let len = u32::from_be_bytes(header) as usize;
    if len > MAX_FRAME {
        return Err(NetError::TooLarge(len));
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;
    serde_json::from_slice(&body).map_err(|e| NetError::Malformed(e.to_string()))
}

/// A byte stream that carries framed messages both ways.
pub struct Channel<S> {
    stream: S,
}

impl<S: AsyncRead + AsyncWrite + Unpin> Channel<S> {
    /// Frames messages over `stream`.
    pub fn new(stream: S) -> Self {
        Self { stream }
    }

    /// Sends one message.
    pub async fn send<T: Serialize>(&mut self, message: &T) -> Result<(), NetError> {
        write_frame(&mut self.stream, message).await
    }

    /// Waits for the next message.
    pub async fn recv<T: DeserializeOwned>(&mut self) -> Result<T, NetError> {
        read_frame(&mut self.stream).await
    }

    /// The underlying stream.
    pub fn into_inner(self) -> S {
        self.stream
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use tokio::io::duplex;

    use super::*;

    #[tokio::test]
    async fn a_message_round_trips_with_a_big_endian_length_prefix() {
        let (mut a, mut b) = duplex(1024);
        write_frame(&mut a, &json!({"hi": 1})).await.unwrap();
        let mut raw = [0u8; 4];
        b.read_exact(&mut raw).await.unwrap();
        assert_eq!(u32::from_be_bytes(raw), 8);
        let mut body = [0u8; 8];
        b.read_exact(&mut body).await.unwrap();
        assert_eq!(&body, br#"{"hi":1}"#);

        write_frame(&mut a, &json!([1, 2])).await.unwrap();
        let got: Value = read_frame(&mut b).await.unwrap();
        assert_eq!(got, json!([1, 2]));
    }

    #[tokio::test]
    async fn a_frame_over_the_limit_is_refused_from_its_header_alone() {
        let (mut a, mut b) = duplex(64);
        let header = u32::try_from(MAX_FRAME + 1).unwrap().to_be_bytes();
        a.write_all(&header).await.unwrap();
        let got: Result<Value, _> = read_frame(&mut b).await;
        assert!(matches!(got, Err(NetError::TooLarge(n)) if n == MAX_FRAME + 1));
    }

    #[tokio::test]
    async fn a_frame_exactly_at_the_limit_is_allowed_and_one_over_is_not_sent() {
        let (mut a, mut b) = duplex(MAX_FRAME + 64);
        // `"` + payload + `"` is the JSON string.
        let at_limit = Value::String("x".repeat(MAX_FRAME - 2));
        write_frame(&mut a, &at_limit).await.unwrap();
        let back: Value = read_frame(&mut b).await.unwrap();
        assert_eq!(back, at_limit);

        let over = Value::String("x".repeat(MAX_FRAME - 1));
        assert!(matches!(
            write_frame(&mut a, &over).await,
            Err(NetError::TooLarge(_))
        ));
    }

    #[tokio::test]
    async fn closing_between_frames_is_closed_and_garbage_is_malformed() {
        let (mut a, mut b) = duplex(64);
        a.write_all(&3u32.to_be_bytes()).await.unwrap();
        a.write_all(b"{{{").await.unwrap();
        let got: Result<Value, _> = read_frame(&mut b).await;
        assert!(matches!(got, Err(NetError::Malformed(_))));
        drop(a);
        let got: Result<Value, _> = read_frame(&mut b).await;
        assert!(matches!(got, Err(NetError::Closed)));
    }
}
