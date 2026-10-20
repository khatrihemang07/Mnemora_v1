import type { AppError, ErrorCode } from './bindings';

type Result<T> = { status: 'ok'; data: T } | { status: 'error'; error: AppError };

/** The error the Rust side reported, as a real Error so TanStack Query and try/catch see it. */
export class IpcError extends Error {
  readonly code: ErrorCode;
  readonly details: string | null;

  constructor(error: AppError) {
    super(error.message);
    this.name = 'IpcError';
    this.code = error.code;
    this.details = error.details;
  }
}

export async function unwrap<T>(call: Promise<Result<T>>): Promise<T> {
  const result = await call;
  if (result.status === 'error') throw new IpcError(result.error);
  return result.data;
}
