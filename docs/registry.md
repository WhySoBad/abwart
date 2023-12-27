# Registry

>[!IMPORTANT]
> Currently abwart only supports `distribution/distribution` registries from version `v2.4.0` and above due to the reliance onto the 
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