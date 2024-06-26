# Registry

>[!IMPORTANT]
> Currently abwart only supports registries running the `registry` image from version `v2.4.0` and above due to the reliance onto the 
> built in garbage collector. Support for other registries is not planned.

A registry consists of a set of configurations. Most of the configuration options are [policies](policies.md) but there are a few registry-specific options as well:
* `enable`: Only registries with `enable` set to `true` get managed by abwart
* `username`, `password`: These optional fields are used for http basic auth when provided. <br>
**Important:** Both fields have to be provided in order to use basic auth
* `network`: The network over which abwart can reach the registry. When deploying abwart and the registry in the same docker-compose you don't need to worry about 
this field. <br>
It can be used to have one central abwart instance running with a specific network (e.g. `abwart-net`). All registries which should be
managed by abwart then have to be in the same network and specify the name of the network in the `network` configuration field
* `port`: The port on which the registry is reachable. By default, the registry api is expected to be available at port `5000`
* `cleanup`: A cron schedule which specifies an interval in which the garbage collector should be run in the registry independent from any rules <br>
This is especially useful when pushing images under the same tag in a CI/CD pipeline. In such a scenario the revision count for the image isn't increasing
since the tag is simply overwritten. This can cause abwart to not trigger any deletions which can lead to big dangling binary blobs. <br>
The `cleanup` field expects the same syntax as the `schedule` field described in the documentation about [rules](rule.md).

>[!CAUTION]
> The garbage collector shipped with the `registry` image breaks schema 2 manifest list and the OCI image index which causes the images to be corrupted. 
> The pull request resolving this issue was already merged in the `distribution/distribution` repository but not yet released. The progress of the issue is tracked in 
> this [issue](https://github.com/WhySoBad/abwart/issues/2)
> 
> Until a registry containing the fixed garbage collector is released the use of the `cleanup` flag is **discouraged** as it potentially breaks your images 


>[!IMPORTANT]
> If `tidy` is set to `false` on every rule and no `cleanup` schedule is provided the blobs marked for deletion aren't actually deleted from the registry

## Defaults

A registry can have default values to which policies without explicit definition in a rule fall back to.
All policies without an explicit default value for the registry fall back to the **global default** which is defined on a per-policy basis. 
The global default of each policy is mentioned in the documentation about [policies](policies.md).

The `default` on a registry can hold all fields a rule can hold (schedule, policies, ...). 

Default values can be assigned to a registry using fields in the following format: `default.<policy_name>`


## Rules

A registry can have multiple rules. Rules allow you to create a fine-grained combination of policies which can run at a customized schedule.
More about rules can be read in the documentation about [rules](rule.md).

Rules can be assigned to a registry using fields in the following format: `rule.<rule_name>.<policy_name>`