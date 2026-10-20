# sv-router behind our own wrapper

Routing uses sv-router in history mode, but the app only touches it through `src/lib/router`, which builds routes from Module manifests, exposes `navigate`, `href`, `isActive` and `blockNavigation`, and wires the Android back button. The wrapper keeps the library swappable and the route table owned by the manifests.
