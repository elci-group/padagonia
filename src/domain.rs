//! Dreamsequence control-plane ontology names.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeKind {
    Account,
    Workspace,
    Principal,
    Device,
    Run,
    Source,
    Repository,
    Pattern,
    Opportunity,
    Capability,
    InferenceRequest,
    WebhookReceipt,
    AuditEvent,
    Subscription,
}

impl NodeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Account => "Account",
            Self::Workspace => "Workspace",
            Self::Principal => "Principal",
            Self::Device => "Device",
            Self::Run => "Run",
            Self::Source => "Source",
            Self::Repository => "Repository",
            Self::Pattern => "Pattern",
            Self::Opportunity => "Opportunity",
            Self::Capability => "Capability",
            Self::InferenceRequest => "InferenceRequest",
            Self::WebhookReceipt => "WebhookReceipt",
            Self::AuditEvent => "AuditEvent",
            Self::Subscription => "Subscription",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationKind {
    Contains,
    MemberOf,
    PairedTo,
    Emitted,
    ObservedIn,
    BelongsTo,
    Repeats,
    Supports,
    Suggests,
    Extends,
    Implements,
    ValidatedBy,
    ReleasedAs,
    ChargedTo,
    Supersedes,
}

impl RelationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::MemberOf => "member_of",
            Self::PairedTo => "paired_to",
            Self::Emitted => "emitted",
            Self::ObservedIn => "observed_in",
            Self::BelongsTo => "belongs_to",
            Self::Repeats => "repeats",
            Self::Supports => "supports",
            Self::Suggests => "suggests",
            Self::Extends => "extends",
            Self::Implements => "implements",
            Self::ValidatedBy => "validated_by",
            Self::ReleasedAs => "released_as",
            Self::ChargedTo => "charged_to",
            Self::Supersedes => "supersedes",
        }
    }
}

pub const REQUIRED_NODE_KINDS: &[&str] = &[
    "Account",
    "Workspace",
    "Principal",
    "Device",
    "Run",
    "Source",
    "Repository",
    "Pattern",
    "Opportunity",
    "Capability",
    "InferenceRequest",
    "WebhookReceipt",
    "AuditEvent",
    "Subscription",
];
pub const REQUIRED_RELATION_KINDS: &[&str] = &[
    "contains",
    "member_of",
    "paired_to",
    "emitted",
    "observed_in",
    "belongs_to",
    "repeats",
    "supports",
    "suggests",
    "extends",
    "implements",
    "validated_by",
    "released_as",
    "charged_to",
    "supersedes",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ontology_covers_directive_kinds() {
        assert_eq!(REQUIRED_NODE_KINDS.len(), 14);
        assert_eq!(REQUIRED_RELATION_KINDS.len(), 15);
        assert_eq!(NodeKind::Run.as_str(), "Run");
        assert_eq!(RelationKind::Supersedes.as_str(), "supersedes");
    }
}
