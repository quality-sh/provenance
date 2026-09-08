// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum NeighborsSuccessOutputGraphNode {
    Variant0 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        commit_pin: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<
            NeighborsSuccessOutputDeclarationAddress,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        effective_date: ::std::option::Option<i64>,
        id: NeighborsSuccessOutputStableId,
        name: ::std::string::String,
        node_type: NeighborsSuccessOutputGraphNodeVariant0NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<NeighborsSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<NeighborsSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        reference: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        review_date: ::std::option::Option<i64>,
        schema_version: u32,
        scope_id: NeighborsSuccessOutputScopeId,
        source_type: NeighborsSuccessOutputSourceType,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<NeighborsSuccessOutputStableId>,
        url: ::std::option::Option<::std::string::String>,
    },
    Variant1 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<
            NeighborsSuccessOutputDeclarationAddress,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        depends_on: ::std::vec::Vec<NeighborsSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        domain_id: ::std::option::Option<NeighborsSuccessOutputStableId>,
        /**Deliberately unstructured free text: the dim view of decisions and
investigations that are coming but cannot yet be phrased sharply.*/
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        fog: ::std::option::Option<::std::string::String>,
        id: NeighborsSuccessOutputStableId,
        node_type: NeighborsSuccessOutputGraphNodeVariant1NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<NeighborsSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<NeighborsSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        refines: ::std::option::Option<NeighborsSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        schema_version: u32,
        scope_id: NeighborsSuccessOutputScopeId,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        source_refs: ::std::vec::Vec<NeighborsSuccessOutputSourceReference>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        spawned_by: ::std::option::Option<NeighborsSuccessOutputStableId>,
        statement: ::std::string::String,
        status: NeighborsSuccessOutputRequirementStatus,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<NeighborsSuccessOutputStableId>,
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
        id: NeighborsSuccessOutputStableId,
        inputs: ::std::vec::Vec<NeighborsSuccessOutputResolutionInput>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        made_by: ::std::option::Option<::std::string::String>,
        node_type: NeighborsSuccessOutputGraphNodeVariant2NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<NeighborsSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<NeighborsSuccessOutputStableId>,
        position: ::std::string::String,
        rationale: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        requirement_ids: ::std::vec::Vec<NeighborsSuccessOutputStableId>,
        review_on: ::std::option::Option<::std::string::String>,
        schema_version: u32,
        scope_id: NeighborsSuccessOutputScopeId,
        status: NeighborsSuccessOutputResolutionStatus,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<NeighborsSuccessOutputStableId>,
        title: ::std::string::String,
    },
    Variant3 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<
            NeighborsSuccessOutputDeclarationAddress,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        id: NeighborsSuccessOutputStableId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        name: ::std::option::Option<::std::string::String>,
        node_type: NeighborsSuccessOutputGraphNodeVariant3NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<NeighborsSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<NeighborsSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        requirement_ids: ::std::vec::Vec<NeighborsSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        resolution_ids: ::std::vec::Vec<NeighborsSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        schema_version: u32,
        scope_id: NeighborsSuccessOutputScopeId,
        severity: NeighborsSuccessOutputRuleSeverity,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_document: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_section: ::std::option::Option<::std::string::String>,
        statement: ::std::string::String,
        status: NeighborsSuccessOutputRuleStatus,
    },
    Variant4 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        claimed_at: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        claimed_by: ::std::option::Option<::std::string::String>,
        id: NeighborsSuccessOutputStableId,
        links: ::std::vec::Vec<NeighborsSuccessOutputArtifactLink>,
        node_type: NeighborsSuccessOutputGraphNodeVariant4NodeType,
        requirement_id: NeighborsSuccessOutputStableId,
        schema_version: u32,
        scope_id: NeighborsSuccessOutputScopeId,
        status: NeighborsSuccessOutputTopicStatus,
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
        contradicts: ::std::option::Option<NeighborsSuccessOutputStableId>,
        id: NeighborsSuccessOutputStableId,
        links: ::std::vec::Vec<NeighborsSuccessOutputArtifactLink>,
        node_type: NeighborsSuccessOutputGraphNodeVariant5NodeType,
        question: ::std::string::String,
        requirement_id: NeighborsSuccessOutputStableId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        resolution_id: ::std::option::Option<NeighborsSuccessOutputStableId>,
        ///The verb that resolves this question, chosen when the question is minted.
        resolution_method: NeighborsSuccessOutputResolutionMethod,
        schema_version: u32,
        scope_id: NeighborsSuccessOutputScopeId,
        status: NeighborsSuccessOutputQuestionStatus,
        topic_id: NeighborsSuccessOutputStableId,
    },
    Variant6 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        color: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        id: NeighborsSuccessOutputStableId,
        name: ::std::string::String,
        node_type: NeighborsSuccessOutputGraphNodeVariant6NodeType,
        schema_version: u32,
        scope_id: NeighborsSuccessOutputScopeId,
    },
    Variant7 {
        id: NeighborsSuccessOutputStableId,
        node_type: NeighborsSuccessOutputGraphNodeVariant7NodeType,
        requirement_id: NeighborsSuccessOutputStableId,
        schema_version: u32,
        scope_id: NeighborsSuccessOutputScopeId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_ref: ::std::option::Option<NeighborsSuccessOutputSourceReference>,
        statement: ::std::string::String,
    },
}
