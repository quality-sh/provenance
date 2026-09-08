// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum GetSuccessOutputGraphNode {
    Variant0 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        commit_pin: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<GetSuccessOutputDeclarationAddress>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        effective_date: ::std::option::Option<i64>,
        id: GetSuccessOutputStableId,
        name: ::std::string::String,
        node_type: GetSuccessOutputGraphNodeVariant0NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<GetSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<GetSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        reference: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        review_date: ::std::option::Option<i64>,
        schema_version: u32,
        scope_id: GetSuccessOutputScopeId,
        source_type: GetSuccessOutputSourceType,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<GetSuccessOutputStableId>,
        url: ::std::option::Option<::std::string::String>,
    },
    Variant1 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<GetSuccessOutputDeclarationAddress>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        depends_on: ::std::vec::Vec<GetSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        domain_id: ::std::option::Option<GetSuccessOutputStableId>,
        /**Deliberately unstructured free text: the dim view of decisions and
investigations that are coming but cannot yet be phrased sharply.*/
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        fog: ::std::option::Option<::std::string::String>,
        id: GetSuccessOutputStableId,
        node_type: GetSuccessOutputGraphNodeVariant1NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<GetSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<GetSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        refines: ::std::option::Option<GetSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        schema_version: u32,
        scope_id: GetSuccessOutputScopeId,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        source_refs: ::std::vec::Vec<GetSuccessOutputSourceReference>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        spawned_by: ::std::option::Option<GetSuccessOutputStableId>,
        statement: ::std::string::String,
        status: GetSuccessOutputRequirementStatus,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<GetSuccessOutputStableId>,
    },
    Variant2 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        approved_at: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        approved_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        confidence: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        context: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        enforcement: ::std::option::Option<::std::string::String>,
        id: GetSuccessOutputStableId,
        inputs: ::std::vec::Vec<GetSuccessOutputResolutionInput>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        made_by: ::std::option::Option<::std::string::String>,
        node_type: GetSuccessOutputGraphNodeVariant2NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<GetSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<GetSuccessOutputStableId>,
        position: ::std::string::String,
        rationale: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        requirement_ids: ::std::vec::Vec<GetSuccessOutputStableId>,
        review_on: ::std::option::Option<::std::string::String>,
        schema_version: u32,
        scope_id: GetSuccessOutputScopeId,
        status: GetSuccessOutputResolutionStatus,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<GetSuccessOutputStableId>,
        title: ::std::string::String,
    },
    Variant3 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<GetSuccessOutputDeclarationAddress>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        id: GetSuccessOutputStableId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        name: ::std::option::Option<::std::string::String>,
        node_type: GetSuccessOutputGraphNodeVariant3NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<GetSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<GetSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        requirement_ids: ::std::vec::Vec<GetSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        resolution_ids: ::std::vec::Vec<GetSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        schema_version: u32,
        scope_id: GetSuccessOutputScopeId,
        severity: GetSuccessOutputRuleSeverity,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_document: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_section: ::std::option::Option<::std::string::String>,
        statement: ::std::string::String,
        status: GetSuccessOutputRuleStatus,
    },
    Variant4 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        claimed_at: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        claimed_by: ::std::option::Option<::std::string::String>,
        id: GetSuccessOutputStableId,
        links: ::std::vec::Vec<GetSuccessOutputArtifactLink>,
        node_type: GetSuccessOutputGraphNodeVariant4NodeType,
        requirement_id: GetSuccessOutputStableId,
        schema_version: u32,
        scope_id: GetSuccessOutputScopeId,
        status: GetSuccessOutputTopicStatus,
        title: ::std::string::String,
    },
    Variant5 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        answer: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        claimed_at: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        claimed_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        contradicts: ::std::option::Option<GetSuccessOutputStableId>,
        id: GetSuccessOutputStableId,
        links: ::std::vec::Vec<GetSuccessOutputArtifactLink>,
        node_type: GetSuccessOutputGraphNodeVariant5NodeType,
        question: ::std::string::String,
        requirement_id: GetSuccessOutputStableId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        resolution_id: ::std::option::Option<GetSuccessOutputStableId>,
        ///The verb that resolves this question, chosen when the question is minted.
        resolution_method: GetSuccessOutputResolutionMethod,
        schema_version: u32,
        scope_id: GetSuccessOutputScopeId,
        status: GetSuccessOutputQuestionStatus,
        topic_id: GetSuccessOutputStableId,
    },
    Variant6 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        color: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        id: GetSuccessOutputStableId,
        name: ::std::string::String,
        node_type: GetSuccessOutputGraphNodeVariant6NodeType,
        schema_version: u32,
        scope_id: GetSuccessOutputScopeId,
    },
    Variant7 {
        id: GetSuccessOutputStableId,
        node_type: GetSuccessOutputGraphNodeVariant7NodeType,
        requirement_id: GetSuccessOutputStableId,
        schema_version: u32,
        scope_id: GetSuccessOutputScopeId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_ref: ::std::option::Option<GetSuccessOutputSourceReference>,
        statement: ::std::string::String,
    },
}
