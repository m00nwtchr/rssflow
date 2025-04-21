#!/bin/zsh

NAME=rssflow

podman build . -t "m00nwtchr/$NAME"

podman save "m00nwtchr/$NAME" | podman -r load

# shellcheck disable=SC2015
podman -r stop "$NAME" || true && podman -r rm "$NAME" || true
podman -r run -d --restart unless-stopped --network web --name "$NAME" -v "${NAME}_data:/data" --env RUST_LOG=info "localhost/m00nwtchr/${NAME}:latest"

#{"nodes":[{"type":"Feed","url":"https://www.azaleaellis.com/tag/pgts/feed/atom","ttl":3600},{"type":"Seen","store":"Internal"},{"type":"Filter","field":"Summary","filter":{"contains":"BELOW IS A SNEAK PEEK OF THIS CONTENT!"},"invert":true},{"type":"Retrieve","content":".entry-content"},{"type":"Sanitise","field":"Content"}]}
