### New Features

#### Model downloads reuse the manifest

`Model::download` no longer fetches the artifact manifest on every call.
A cached manifest is now stored next to the models `.manifest-cache.json` and serves every model until the validity window returned by the artifact server expires.

Manifest requests now give up after 30 seconds. They previously had no timeout and could hang for
as long as the operating system kept retrying the connection.
