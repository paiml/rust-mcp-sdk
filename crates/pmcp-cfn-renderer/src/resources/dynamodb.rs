//! `AWS::DynamoDB::Table` rendering.
//!
//! Today the only caller is `resources::cognito` (the DCR `ClientsTable` —
//! see that module's doc comment), but this module is deliberately
//! descriptor-section-agnostic: [`render_table`] takes a name and a
//! partition-key spec, never a [`pmcp_package::package::DeployDescriptor`].
//! That keeps it the natural landing zone for a future
//! `[[resources.dynamodb]]` descriptor section — a second caller extends
//! this module's signatures (e.g. a sort key, a non-`PAY_PER_REQUEST`
//! billing mode) when it actually needs to, not before (YAGNI).

use crate::{logical_ids, template::CfnResource};
use serde_json::json;

/// Render a single hash-key `AWS::DynamoDB::Table`: `(logical_id, resource)`.
///
/// `name` is used both as the literal `TableName` and (via
/// [`logical_ids::for_table`]) to derive the logical id. `partition_key` is
/// `(attribute_name, attribute_type)`, where `attribute_type` is one of
/// CFN's `AttributeType` codes (`"S"`/`"N"`/`"B"`) — this module does not
/// validate the code; an invalid one simply fails at `cdk`/CloudFormation
/// deploy time, consistent with this crate's general posture of trusting
/// already-parsed input.
///
/// `BillingMode` is always `PAY_PER_REQUEST` and point-in-time recovery is
/// always enabled — fixed today (matching the DCR `ClientsTable`, this
/// module's only real caller); a future `[[resources.dynamodb]]` caller
/// that needs a different billing mode extends this signature then.
/// `tags` is supplied by the caller (rather than computed here) so this
/// module never needs to know which stack shape — and therefore which tag
/// set/component marker — it is being rendered for.
#[must_use]
pub fn render_table(
    name: &str,
    partition_key: (&str, &str),
    tags: serde_json::Value,
) -> (String, CfnResource) {
    let (key_name, key_type) = partition_key;
    let properties = json!({
        "TableName": name,
        "AttributeDefinitions": [
            { "AttributeName": key_name, "AttributeType": key_type },
        ],
        "KeySchema": [
            { "AttributeName": key_name, "KeyType": "HASH" },
        ],
        "BillingMode": "PAY_PER_REQUEST",
        "PointInTimeRecoverySpecification": { "PointInTimeRecoveryEnabled": true },
        "Tags": tags,
    });
    (
        logical_ids::for_table(name),
        CfnResource {
            type_: "AWS::DynamoDB::Table".to_string(),
            properties,
            depends_on: vec![],
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn logical_id_is_derived_from_the_table_name() {
        let (id, _) = render_table("my-server-oauth-clients", ("client_id", "S"), json!([]));
        assert_eq!(id, "MyServerOauthClientsTable");
    }

    #[test]
    fn table_name_is_the_literal_name_argument() {
        let (_, table) = render_table("my-server-oauth-clients", ("client_id", "S"), json!([]));
        assert_eq!(table.properties["TableName"], "my-server-oauth-clients");
    }

    #[test]
    fn partition_key_drives_attribute_definitions_and_key_schema() {
        let (_, table) = render_table("clients", ("client_id", "S"), json!([]));
        assert_eq!(
            table.properties["AttributeDefinitions"],
            json!([{ "AttributeName": "client_id", "AttributeType": "S" }])
        );
        assert_eq!(
            table.properties["KeySchema"],
            json!([{ "AttributeName": "client_id", "KeyType": "HASH" }])
        );
    }

    #[test]
    fn billing_mode_is_always_pay_per_request() {
        let (_, table) = render_table("clients", ("client_id", "S"), json!([]));
        assert_eq!(table.properties["BillingMode"], "PAY_PER_REQUEST");
    }

    #[test]
    fn point_in_time_recovery_is_always_enabled() {
        let (_, table) = render_table("clients", ("client_id", "S"), json!([]));
        assert_eq!(
            table.properties["PointInTimeRecoverySpecification"],
            json!({ "PointInTimeRecoveryEnabled": true })
        );
    }

    #[test]
    fn tags_pass_through_from_the_caller() {
        let tags = json!([{ "Key": "component", "Value": "oauth" }]);
        let (_, table) = render_table("clients", ("client_id", "S"), tags.clone());
        assert_eq!(table.properties["Tags"], tags);
    }

    #[test]
    fn resource_type_is_dynamodb_table() {
        let (_, table) = render_table("clients", ("client_id", "S"), json!([]));
        assert_eq!(table.type_, "AWS::DynamoDB::Table");
    }

    #[test]
    fn depends_on_is_empty() {
        let (_, table) = render_table("clients", ("client_id", "S"), json!([]));
        assert!(table.depends_on.is_empty());
    }
}
