# Todo widget is Android 12+ only

The Todo widget uses the Android 12 (API 31) remote-views features: collection items built in the update call itself (`RemoteCollectionItems`, no `RemoteViewsService`), size-responsive layouts and the system corner-radius and dynamic-colour resources. It is offered only there: the provider is disabled by default and enabled by a version-qualified resource (`values-v31`), so on Android 8-11 nothing half-working appears in the picker. The app's minimum Android version is unchanged.

Supporting 8-11 would need a `RemoteViewsService` and factory, a second code path for sizes and corners, and no dynamic colours; for a phone that is already on 15 that cost buys nothing.

## Considered Options

- **Support Android 8-11 with a `RemoteViewsService`**: rejected; a second rendering path to maintain and test, for devices nobody here uses.
- **Show the Todo widget everywhere but degrade on old versions**: rejected; a widget that works differently per version is harder to reason about than one that is not offered.
- **Compose Glance**: rejected; it adds a large dependency and build cost on an 8 GB machine, and the RemoteViews API is enough.
