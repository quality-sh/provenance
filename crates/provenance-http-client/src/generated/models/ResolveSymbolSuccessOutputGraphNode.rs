// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum ResolveSymbolSuccessOutputGraphNode {
    Variant0 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        commit_pin: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<
            ResolveSymbolSuccessOutputDeclarationAddress,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        effective_date: ::std::option::Option<i64>,
        id: ResolveSymbolSuccessOutputStableId,
        name: ::std::string::String,
        node_type: ResolveSymbolSuccessOutputGraphNodeVariant0NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        reference: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        review_date: ::std::option::Option<i64>,
        schema_version: u32,
        scope_id: ResolveSymbolSuccessOutputScopeId,
        source_type: ResolveSymbolSuccessOutputSourceType,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<ResolveSymbolSuccessOutputStableId>,
        url: ::std::option::Option<::std::string::String>,
    },
    Variant1 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<
            ResolveSymbolSuccessOutputDeclarationAddress,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        depends_on: ::std::vec::Vec<ResolveSymbolSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        domain_id: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
        /**Deliberately unstructured free text: the dim view of decisions and
investigations that are coming but cannot yet be phrased sharply.*/
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        fog: ::std::option::Option<::std::string::String>,
        id: ResolveSymbolSuccessOutputStableId,
        node_type: ResolveSymbolSuccessOutputGraphNodeVariant1NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        refines: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        schema_version: u32,
        scope_id: ResolveSymbolSuccessOutputScopeId,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        source_refs: ::std::vec::Vec<ResolveSymbolSuccessOutputSourceReference>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        spawned_by: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
        statement: ::std::string::String,
        status: ResolveSymbolSuccessOutputRequirementStatus,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<ResolveSymbolSuccessOutputStableId>,
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
        id: ResolveSymbolSuccessOutputStableId,
        inputs: ::std::vec::Vec<ResolveSymbolSuccessOutputResolutionInput>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        made_by: ::std::option::Option<::std::string::String>,
        node_type: ResolveSymbolSuccessOutputGraphNodeVariant2NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
        position: ::std::string::String,
        rationale: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        requirement_ids: ::std::vec::Vec<ResolveSymbolSuccessOutputStableId>,
        review_on: ::std::option::Option<::std::string::String>,
        schema_version: u32,
        scope_id: ResolveSymbolSuccessOutputScopeId,
        status: ResolveSymbolSuccessOutputResolutionStatus,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        supersedes: ::std::vec::Vec<ResolveSymbolSuccessOutputStableId>,
        title: ::std::string::String,
    },
    Variant3 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declaration_address: ::std::option::Option<
            ResolveSymbolSuccessOutputDeclarationAddress,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        declared_by: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        id: ResolveSymbolSuccessOutputStableId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        name: ::std::option::Option<::std::string::String>,
        node_type: ResolveSymbolSuccessOutputGraphNodeVariant3NodeType,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_message: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        origin_thread: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        requirement_ids: ::std::vec::Vec<ResolveSymbolSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        resolution_ids: ::std::vec::Vec<ResolveSymbolSuccessOutputStableId>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        retired: ::std::option::Option<bool>,
        schema_version: u32,
        scope_id: ResolveSymbolSuccessOutputScopeId,
        severity: ResolveSymbolSuccessOutputRuleSeverity,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_document: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_section: ::std::option::Option<::std::string::String>,
        statement: ::std::string::String,
        status: ResolveSymbolSuccessOutputRuleStatus,
    },
    Variant4 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        claimed_at: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        claimed_by: ::std::option::Option<::std::string::String>,
        id: ResolveSymbolSuccessOutputStableId,
        links: ::std::vec::Vec<ResolveSymbolSuccessOutputArtifactLink>,
        node_type: ResolveSymbolSuccessOutputGraphNodeVariant4NodeType,
        requirement_id: ResolveSymbolSuccessOutputStableId,
        schema_version: u32,
        scope_id: ResolveSymbolSuccessOutputScopeId,
        status: ResolveSymbolSuccessOutputTopicStatus,
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
        contradicts: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
        id: ResolveSymbolSuccessOutputStableId,
        links: ::std::vec::Vec<ResolveSymbolSuccessOutputArtifactLink>,
        node_type: ResolveSymbolSuccessOutputGraphNodeVariant5NodeType,
        question: ::std::string::String,
        requirement_id: ResolveSymbolSuccessOutputStableId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        resolution_id: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
        ///The verb that resolves this question, chosen when the question is minted.
        resolution_method: ResolveSymbolSuccessOutputResolutionMethod,
        schema_version: u32,
        scope_id: ResolveSymbolSuccessOutputScopeId,
        status: ResolveSymbolSuccessOutputQuestionStatus,
        topic_id: ResolveSymbolSuccessOutputStableId,
    },
    Variant6 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        color: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        description: ::std::option::Option<::std::string::String>,
        id: ResolveSymbolSuccessOutputStableId,
        name: ::std::string::String,
        node_type: ResolveSymbolSuccessOutputGraphNodeVariant6NodeType,
        schema_version: u32,
        scope_id: ResolveSymbolSuccessOutputScopeId,
    },
    Variant7 {
        id: ResolveSymbolSuccessOutputStableId,
        node_type: ResolveSymbolSuccessOutputGraphNodeVariant7NodeType,
        requirement_id: ResolveSymbolSuccessOutputStableId,
        schema_version: u32,
        scope_id: ResolveSymbolSuccessOutputScopeId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        source_ref: ::std::option::Option<ResolveSymbolSuccessOutputSourceReference>,
        statement: ::std::string::String,
    },
}
