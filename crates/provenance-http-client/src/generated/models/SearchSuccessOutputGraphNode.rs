// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum SearchSuccessOutputGraphNode {
    Variant0 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        commit_pin: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<
            SearchSuccessOutputDeclarationAddress,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        effective_date: ::std::option::Option<i64>,
        id: SearchSuccessOutputStableId,
        name: ::std::string::String,
        node_type: SearchSuccessOutputGraphNodeVariant0NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<SearchSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<SearchSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        reference: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        review_date: ::std::option::Option<i64>,
        schema_version: u32,
        scope_id: SearchSuccessOutputScopeId,
        source_type: SearchSuccessOutputSourceType,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<SearchSuccessOutputStableId>,
        url: ::std::option::Option<::std::string::String>,
    },
    Variant1 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<
            SearchSuccessOutputDeclarationAddress,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        depends_on: ::std::vec::Vec<SearchSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        domain_id: ::std::option::Option<SearchSuccessOutputStableId>,
        /**Deliberately unstructured free text: the dim view of decisions and
investigations that are coming but cannot yet be phrased sharply.*/
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        fog: ::std::option::Option<::std::string::String>,
        id: SearchSuccessOutputStableId,
        node_type: SearchSuccessOutputGraphNodeVariant1NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<SearchSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<SearchSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        refines: ::std::option::Option<SearchSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        schema_version: u32,
        scope_id: SearchSuccessOutputScopeId,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        source_refs: ::std::vec::Vec<SearchSuccessOutputSourceReference>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        spawned_by: ::std::option::Option<SearchSuccessOutputStableId>,
        statement: ::std::string::String,
        status: SearchSuccessOutputRequirementStatus,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<SearchSuccessOutputStableId>,
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
        id: SearchSuccessOutputStableId,
        inputs: ::std::vec::Vec<SearchSuccessOutputResolutionInput>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        made_by: ::std::option::Option<::std::string::String>,
        node_type: SearchSuccessOutputGraphNodeVariant2NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<SearchSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<SearchSuccessOutputStableId>,
        position: ::std::string::String,
        rationale: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        requirement_ids: ::std::vec::Vec<SearchSuccessOutputStableId>,
        review_on: ::std::option::Option<::std::string::String>,
        schema_version: u32,
        scope_id: SearchSuccessOutputScopeId,
        status: SearchSuccessOutputResolutionStatus,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<SearchSuccessOutputStableId>,
        title: ::std::string::String,
    },
    Variant3 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<
            SearchSuccessOutputDeclarationAddress,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        id: SearchSuccessOutputStableId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        name: ::std::option::Option<::std::string::String>,
        node_type: SearchSuccessOutputGraphNodeVariant3NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<SearchSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<SearchSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        requirement_ids: ::std::vec::Vec<SearchSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        resolution_ids: ::std::vec::Vec<SearchSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        schema_version: u32,
        scope_id: SearchSuccessOutputScopeId,
        severity: SearchSuccessOutputRuleSeverity,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_document: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_section: ::std::option::Option<::std::string::String>,
        statement: ::std::string::String,
        status: SearchSuccessOutputRuleStatus,
    },
    Variant4 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        claimed_at: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        claimed_by: ::std::option::Option<::std::string::String>,
        id: SearchSuccessOutputStableId,
        links: ::std::vec::Vec<SearchSuccessOutputArtifactLink>,
        node_type: SearchSuccessOutputGraphNodeVariant4NodeType,
        requirement_id: SearchSuccessOutputStableId,
        schema_version: u32,
        scope_id: SearchSuccessOutputScopeId,
        status: SearchSuccessOutputTopicStatus,
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
        contradicts: ::std::option::Option<SearchSuccessOutputStableId>,
        id: SearchSuccessOutputStableId,
        links: ::std::vec::Vec<SearchSuccessOutputArtifactLink>,
        node_type: SearchSuccessOutputGraphNodeVariant5NodeType,
        question: ::std::string::String,
        requirement_id: SearchSuccessOutputStableId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        resolution_id: ::std::option::Option<SearchSuccessOutputStableId>,
        ///The verb that resolves this question, chosen when the question is minted.
        resolution_method: SearchSuccessOutputResolutionMethod,
        schema_version: u32,
        scope_id: SearchSuccessOutputScopeId,
        status: SearchSuccessOutputQuestionStatus,
        topic_id: SearchSuccessOutputStableId,
    },
    Variant6 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        color: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        id: SearchSuccessOutputStableId,
        name: ::std::string::String,
        node_type: SearchSuccessOutputGraphNodeVariant6NodeType,
        schema_version: u32,
        scope_id: SearchSuccessOutputScopeId,
    },
    Variant7 {
        id: SearchSuccessOutputStableId,
        node_type: SearchSuccessOutputGraphNodeVariant7NodeType,
        requirement_id: SearchSuccessOutputStableId,
        schema_version: u32,
        scope_id: SearchSuccessOutputScopeId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_ref: ::std::option::Option<SearchSuccessOutputSourceReference>,
        statement: ::std::string::String,
    },
}
