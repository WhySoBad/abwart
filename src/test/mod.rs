use std::sync::Arc;
use chrono::Duration;
use crate::api::DistributionConfig;
use crate::api::repository::Repository;
use crate::api::tag::Tag;

pub fn get_distribution_config() -> Arc<DistributionConfig> {
    let config = DistributionConfig::new(String::new(), None, None, true);
    Arc::new(config)
}

pub fn get_repositories(names: Vec<impl Into<String>>) -> Vec<Repository> {
    let mut repositories = vec![];
    let config = get_distribution_config();
    for name in names {
        repositories.push(Repository::new(name.into(), config.clone()))
    }
    repositories
}

pub fn get_tags(raw: Vec<(impl Into<String>, Duration, u32)>) -> Vec<Tag> {
    let mut tags = vec![];
    let now = chrono::offset::Utc::now();
    for (name, offset, size) in raw {
        tags.push(Tag::new(name.into(), String::new(), now + offset, size))
    }
    tags
}