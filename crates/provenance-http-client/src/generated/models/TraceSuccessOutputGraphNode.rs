// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum TraceSuccessOutputGraphNode {
    Variant0 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        commit_pin: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<TraceSuccessOutputDeclarationAddress>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        effective_date: ::std::option::Option<i64>,
        id: TraceSuccessOutputStableId,
        name: ::std::string::String,
        node_type: TraceSuccessOutputGraphNodeVariant0NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<TraceSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<TraceSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        reference: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        review_date: ::std::option::Option<i64>,
        schema_version: u32,
        scope_id: TraceSuccessOutputScopeId,
        source_type: TraceSuccessOutputSourceType,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<TraceSuccessOutputStableId>,
        url: ::std::option::Option<::std::string::String>,
    },
    Variant1 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<TraceSuccessOutputDeclarationAddress>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        depends_on: ::std::vec::Vec<TraceSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        domain_id: ::std::option::Option<TraceSuccessOutputStableId>,
        /**Deliberately unstructured free text: the dim view of decisions and
investigations that are coming but cannot yet be phrased sharply.*/
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        fog: ::std::option::Option<::std::string::String>,
        id: TraceSuccessOutputStableId,
        node_type: TraceSuccessOutputGraphNodeVariant1NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<TraceSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<TraceSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        refines: ::std::option::Option<TraceSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        schema_version: u32,
        scope_id: TraceSuccessOutputScopeId,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        source_refs: ::std::vec::Vec<TraceSuccessOutputSourceReference>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        spawned_by: ::std::option::Option<TraceSuccessOutputStableId>,
        statement: ::std::string::String,
        status: TraceSuccessOutputRequirementStatus,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<TraceSuccessOutputStableId>,
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
        id: TraceSuccessOutputStableId,
        inputs: ::std::vec::Vec<TraceSuccessOutputResolutionInput>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        made_by: ::std::option::Option<::std::string::String>,
        node_type: TraceSuccessOutputGraphNodeVariant2NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<TraceSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<TraceSuccessOutputStableId>,
        position: ::std::string::String,
        rationale: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        requirement_ids: ::std::vec::Vec<TraceSuccessOutputStableId>,
        review_on: ::std::option::Option<::std::string::String>,
        schema_version: u32,
        scope_id: TraceSuccessOutputScopeId,
        status: TraceSuccessOutputResolutionStatus,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<TraceSuccessOutputStableId>,
        title: ::std::string::String,
    },
    Variant3 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<TraceSuccessOutputDeclarationAddress>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        id: TraceSuccessOutputStableId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        name: ::std::option::Option<::std::string::String>,
        node_type: TraceSuccessOutputGraphNodeVariant3NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<TraceSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<TraceSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        requirement_ids: ::std::vec::Vec<TraceSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        resolution_ids: ::std::vec::Vec<TraceSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        schema_version: u32,
        scope_id: TraceSuccessOutputScopeId,
        severity: TraceSuccessOutputRuleSeverity,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_document: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_section: ::std::option::Option<::std::string::String>,
        statement: ::std::string::String,
        status: TraceSuccessOutputRuleStatus,
    },
    Variant4 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        claimed_at: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        claimed_by: ::std::option::Option<::std::string::String>,
        id: TraceSuccessOutputStableId,
        links: ::std::vec::Vec<TraceSuccessOutputArtifactLink>,
        node_type: TraceSuccessOutputGraphNodeVariant4NodeType,
        requirement_id: TraceSuccessOutputStableId,
        schema_version: u32,
        scope_id: TraceSuccessOutputScopeId,
        status: TraceSuccessOutputTopicStatus,
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
        contradicts: ::std::option::Option<TraceSuccessOutputStableId>,
        id: TraceSuccessOutputStableId,
        links: ::std::vec::Vec<TraceSuccessOutputArtifactLink>,
        node_type: TraceSuccessOutputGraphNodeVariant5NodeType,
        question: ::std::string::String,
        requirement_id: TraceSuccessOutputStableId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        resolution_id: ::std::option::Option<TraceSuccessOutputStableId>,
        ///The verb that resolves this question, chosen when the question is minted.
        resolution_method: TraceSuccessOutputResolutionMethod,
        schema_version: u32,
        scope_id: TraceSuccessOutputScopeId,
        status: TraceSuccessOutputQuestionStatus,
        topic_id: TraceSuccessOutputStableId,
    },
    Variant6 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        color: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        id: TraceSuccessOutputStableId,
        name: ::std::string::String,
        node_type: TraceSuccessOutputGraphNodeVariant6NodeType,
        schema_version: u32,
        scope_id: TraceSuccessOutputScopeId,
    },
    Variant7 {
        id: TraceSuccessOutputStableId,
        node_type: TraceSuccessOutputGraphNodeVariant7NodeType,
        requirement_id: TraceSuccessOutputStableId,
        schema_version: u32,
        scope_id: TraceSuccessOutputScopeId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_ref: ::std::option::Option<TraceSuccessOutputSourceReference>,
        statement: ::std::string::String,
    },
}
