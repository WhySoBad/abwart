# Configuration

There are two ways of configuring the registries which are manged by abwart. You can either add the configuration for each registry as labels to the 
registry container, or you can have a static configuration file which contains the configuration of multiple registries based off their name.

Configuration with labels can be combined with configuration inside a static configuration file. Should there be overlapping configurations 
the one from **the static configuration file is preferred** over the one defined with labels.

The examples below are the same but one is in label form and one is in static configuration form.

## Labels

The easiest way to configure registries is with [docker object labels](https://docs.docker.com/config/labels-custom-metadata/). It allows to have the configuration
for each repository direct on the associated container instead of relying on a central configuration file. 

> [!IMPORTANT]
> All configurations mentioned in the documentation always have to be prefixed with `abwart.` in label form to prevent collisions with other applications
> 
> e.g. the `default.schedule` configuration would then be `abwart.default.schedule`

### Format

The labels are dot separated chains of configuration or policy fields. 
Unknown [registry](registry.md), [rule](rule.md) or [policy](policies.md) configurations are ignored.

### Example

```shell
docker run \
 --label abwart.enable=true \
 --label abwart.default.schedule="0 2 * * * *" \
 --label abwart.rule.weekend.schedule="0 0 0 * * Sun,Sat" \
 --label abwart.rule.weekend.revisions=10 \
 distribution/distribution:2.8.3
```

Read more about the different configurations:
- [Registry](registry.md)
- [Rule](rule.md)
- [Policies](policies.md)

## Static configuration file

The static configuration file is a file in the **yaml** format which is located at `config.yml` relative to the binary (the path can be overwritten 
using the `CONFIG_PATH` environment variable).

> When running abwart as a docker container the default configuration path inside the container is `/app/config.yml`. You're expected to mount your 
> static configuration file into this location using a bind-mount.

Despite the name the static configuration file is only static in the sense abwart doesn't change it whilst running. 
Actually, the static configuration file has **hot reloading** capabilities. This allows the user to update the configuration of registries
whilst abwart is running. You can even change the configuration for already running registries without causing any downtime on the registry.

The static configuration file associates a configuration with a registry **based off the name** of the registry container. Therefore, you'll have to
explicitly set the name of the container when starting the registry container.

> [!NOTE]
> It's important to note the registry container still needs the `abwart.enable=true` label to prevent unwanted association with a container. 

### Format

The static configuration file expects an object with registry configurations in the field `registries`.
Unknown [registry](registry.md), [rule](rule.md) or [policy](policies.md) configurations are ignored.

In the documentation the configuration fields are referenced in dot-separated form (e.g. `default.schedule`). In the static configuration file 
the dot represents an indentation in the yaml file. Exceptions for this rule are policy identifiers which are dot-separated (e.g. `age.min`).

### Example

```yaml
registries:
  registry-1:
    default:
      schedule: 0 2 * * * *
      age.max: 30d
    rule:
      weekend:
        schedule: 0 0 0 * * Sun,Sat
        revisions: 10
```

Read more about the different configurations:
- [Registry](registry.md)
- [Rule](rule.md)
- [Policies](policies.md)