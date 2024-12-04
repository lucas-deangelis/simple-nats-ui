build-images:
	nix build .#docker --out-link docker-versioned
	nix build .#dockerLatest --out-link docker-latest

load-images:
	docker load < docker-versioned
	docker load < docker-latest

push-images-to-github:
	docker push ghcr.io/lucas-deangelis/simple-nats-ui:latest
	docker push ghcr.io/lucas-deangelis/simple-nats-ui:$(shell grep version Cargo.toml | head -n 1 | cut -d '"' -f 2)
