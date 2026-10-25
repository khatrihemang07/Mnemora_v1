# The AI agent harness is written in Rust, inspired by pi, not pi itself

The AI Module answers questions with a small agent loop: the Model asks for read-only Tools, the harness runs them and feeds the results back, and steering and stopping work between requests. We hand-write that loop in Rust, following the shape of pi's core (typed messages and content blocks, a Provider that streams events and never throws, a nested loop with steering and follow-up queues, and a stop-reason gate), and port Sphinx's JSON envelope for Providers that cannot make native tool calls. The loop is pinned to pi 1.2.0, the version Sphinx embeds. The trade-off is that we own the loop, the wire adapters and the envelope code, and must keep them matching pi's behaviour by hand, in exchange for one Rust core that runs inside the Tauri app on desktop and Android with no Node runtime, and that the command-layer tests can drive with a scripted fake Provider.

## Considered Options

- **Embed pi (`@earendil-works/pi-agent-core` or `pi-coding-agent`)**: rejected; it needs a Node runtime, which Mnemora does not ship on Android, and `pi-coding-agent` is a coding tool with its own sessions and terminal UI that we would have to strip down.
- **Port Sphinx's reviewer loop unchanged**: rejected; it is built around Git and review workspaces, not a read-only Tool set over Journal and Todo, so only its envelope and limits are reused.
- **Depend on a Rust LLM client crate for the loop**: rejected for now; the Provider contract, the stop-reason gate and the envelope are the parts we need to get exactly right, and a crate would bring its own message types that the loop would have to translate.
