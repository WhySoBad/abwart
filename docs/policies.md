# Policies

The policies are the heart of abwart. They allow you to specify which tags on which repositories should be deleted under which circumstances.
We differentiate between two different type of policies: **tag policies** and **repository policies**.

>[!NOTE]
> You can disable any policy by assigning an empty string to it in the configuration
>
> e.g. `rule.example.revisions=""` would no longer match tags based of their revision count

## Affection types

Additionally, every policy is either of affection type `Requirement` or `Target`. In a sense the affection type determines when the policy is applied. `Target` policies
are always applied before `Requirement` policies. The `Target` policies match all tags/repositories according to some kind of condition. The `Requirement` policies then
check whether all matched tags/repositories fulfill the condition of the policy as well and un-matches the entries which don't match it.

>[!TIP]
>Not all original tags which fulfill a `Requirement` policy should be deleted but all tags which should be deleted have to fulfill the `Requirement` policy

Why the affection types of policies are useful can be easily demonstrated on an example: <br>
Assume we have 100 tags in a repository and a revision policy which only keeps the latest 10 revisions of the image. 
Additionally, we have a minimum age policy 
which requires all images to be older than 10 minutes to be deleted. From those 100 only 85 tags are older than 10 minutes.

The revisions policy marks 90 tags for deletion. Due to the minimum age policy being a `Requirement` policy it un-matches
five of tags which were previously marked for deletion since they don't fulfil the minimum age condition.

In a sense the `Requirement` policies are stronger than the `Target` policies.

## Tag policies

Tag policies are used to determine which tags on an image should be marked for deletion

### Revision policy
> Affection type: `Target`
> 
> Identifier: `revisions`
> 
> Default: `15`

The revision policy aims to only keep a specified amount of tags for an image in the registry. In combination with other 
policies it can be less than the specified amount of tags is kept. When there are more tags than specified in the policy it
marks the excess ones for deletion. The tags are marked for deletion from oldest to newest (by creation date). 

```yaml
# Only keep the latest 15 tags of the image
revisions: 15
```

### Max age policy
> Affection type: `Target`
>
> Identifier: `age.max`
>
> Default: `None`

The maximum age policy marks all tags which are older than a given duration for deletion. As duration a string matching
`[0-9]+(ns|us|ms|[smhdwy])` is expected. The [duration_string](https://docs.rs/duration-string/latest/duration_string/) crate is used for parsing the durations.

```yaml
# Mark all tags older than 30 days for deletion 
age.max: 30d
```

### Min age policy
> Affection type: `Requirement`
>
> Identifier: `age.min`
> 
> Default: `None`

The minimum age policy ensures all tags which are deleted are older than a given duration. As duration a string matching
`[0-9]+(ns|us|ms|[smhdwy])` is expected. The [duration_string](https://docs.rs/duration-string/latest/duration_string/) crate is used for parsing the durations.

```yaml
# Would only delete tags which are all older than 10 days
age.min: 10d
```

### Tag pattern policy
> Affection type: `Target`
>
> Identifier: `tag.pattern`
>
> Default: `.+`

The tag pattern policy matches all tags by name against a regex. For regex parsing the [regex](https://docs.rs/regex/latest/regex/) crate is used.
Any valid regex (supported by this crate) can be used.

```yaml
# Would match all tags which end in -beta or -alpha (e.g. frontend-alpha)
tag.pattern: .+-(beta|alpha)
```

## Repository policies

Repository policies are used to determine for which images a rule should be applied

### Image pattern policy
> Affection type: `Target`
> 
> Identifier: `image.pattern`
>
> Default: `.+`

The image pattern policy matches all repositories by name against a regex. For regex parsing the [regex](https://docs.rs/regex/latest/regex/) crate is used.
Any valid regex (supported by this crate) can be used. 

```yaml
# Would match all images which end in -beta or -alpha (e.g. frontend-alpha)
image.pattern: .+-(beta|alpha)
```