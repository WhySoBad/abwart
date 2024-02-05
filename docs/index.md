# Documentation

## Vocabulary
The terms used in this documentation may differ from the terms used in other applications. Therefore, we have the following definitions:
- **image**, **tag**: An image/repository is what one refers to as repository in the context of a docker registry. An image can have multiple tags.
In the case of `registry:2.8.3`, `registry` is the image and `2.8.3` is the tag. Images may be referenced as repositories.
- **instance**: An instance is a container running a docker container registry. Instances may be referenced as registries
- **rule**, **policy**: A policy is a single condition for determining whether an image (or tag) contains (or is) data to be removed. A rule is a collection
of policies together with a name and a schedule

## Quick navigation:
- [Configuration](configuration.md)
- [Registry](registry.md)
- [Policies](policies.md)
- [Rule](rule.md)
