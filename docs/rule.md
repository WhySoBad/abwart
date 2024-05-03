# Rule

A rule is a set of policies together with a name and a schedule. All policies which aren't explicitly specified in a rule fall back to their default value
(either the registry or global default). 

The name of a rule is only for cosmetic reasons so one can easily see what this combination of policies should do.

Every rule can run on a different schedule than the default for the registry. For this the `schedule` field on the rule can be specified. 
The schedule is expected to be a cron expression in the `<second> <minute> <hour> <day of month> <month> <day of week> <year>` format. The [cron](https://github.com/zslayton/cron)
crate is used internally for parsing the cron expressions.

Additionally, a `tidy` flag can be specified for every rule. If at least one policy with `tidy` set to `true` is applied to the registry the garbage collector
will be run directly after the application of the rule. The `tidy` flag only has an effect if set to `true`.

More about the available policies can be read in the documentation about [policies](policies.md).

## Example 

```yaml
# custom schedule for the rule named `example`
rule.example.schedule: 0 * * * * * *
# the `example` rule contains a custom minimum age policy
rule.example.age.min: 30d
# run the garbage collector after this rule was applied
rule.example.tidy: true
```