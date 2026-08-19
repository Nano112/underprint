# Model artifacts

Underprint does not commit TrustMark model weights to this source repository.
`manifest.json` records the upstream location, exact byte size, and SHA-256 of
the compatibility artifacts used during development.

Run `../scripts/fetch-models.sh` to download and verify the two Q models. A
production release must not redistribute them until the model-weight licence
and redistribution permission are recorded explicitly; Adobe's MIT source-code
licence is not silently treated as ownership of the weights.
