# abwart

abwart is a blazing fast housekeeper for your docker container registry written in rust.

You are selfhosting a docker registry and don't want to waste resources to images you no longer need? You prefer the lightweight nature of the
 `distribution/distribution` container registry over other solutions? Then abwart may be of interest for you!
It offers the functionality to add retention policies to your registry with easy-to-use configuration methods.

Features of abwart:
* Easily configurable over container labels or a static configuration file with hot reloading
* Support for multiple fine-grained rules on a per-registry basis
* Support for default policies on a per-registry basis
* Support for docker distribution and OCI images
* Support for docker registries with http basic authentication or without any authentication
* Automatic garbage collection inside the registry after tag deletion
* Easily deployable using docker
* Support for multiple docker registries (who needs this?) with a single deployment

## Installation

The abwart docker image is hosted in the github container registry and can be run like this:
```shell
docker run -v /var/run/docker.sock:/var/run/docker.sock:ro ghcr.io/whysobad/abwart
```

## Example

```yaml
version: '3.8'

services:
  registry:
    image: distribution/distribution:2.8.3
    labels:
      abwart.enable: true
      # override the default tag revision count for images matching the `weekend` rule 
      abwart.rule.weekend.revisions: 2
      # only run the policies of the `weekend` rule in the weekend at midnight
      abwart.rule.weekend.schedule: 0 0 0 * * Sun,Sat
      # only apply policies of the `weekend` rule to images matching the regex pattern
      abwart.rule.weekend.pattern: \w+-(alpha|beta)
      # only keep 10 revisions of all images stored in the registry by default
      abwart.default.revisions: 10
      # apply policies every day at midnight UTC by default
      abwart.default.schedule: 0 0 0 * * *

  abwart:
    image: ghcr.io/whysobad/abwart
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
```

For more policies available have a look at the [docs](docs/index.md)

## Contributing

You're missing a policy after which images/tags could be matched, you have an idea for this project, or you simply want to contribute? Feel free
to open a new issue or contribute to an open issue! 

**PS**: The code for the policies is very modular which should enable an easy addition/modification of policies.

## TODOs
- [ ] Add tests to `Instance`
- [x] Run tests in GitHub Actions
- [x] Publish a new image to ghcr when a new tag is created
- [x] Add static configuration file which takes priority over label configuration. The file should have hot reload
- [x] Add docs
- [ ] Add configuration whether the garbage collector should be run inside the container
- [x] Add tests to rule parsing and rule affections
- [ ] Add ping to registry container in instance creation
- [ ] Add policy to match tags by pattern

## Credits

The initial idea for this project comes from [deckschrubber](https://github.com/fraunhoferfokus/deckschrubber)

The idea for using labels for the configuration and the syntax of the rule labels is heavily inspired by [traefik](https://github.com/traefik/traefik)