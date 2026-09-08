export interface paths {
    "/metadata": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["metadata"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v7/operations/check-statement": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["checkStatement"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v7/operations/evidence": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["evidence"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v7/operations/get": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["get"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v7/operations/impact": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["impact"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v7/operations/info": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["info"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v7/operations/neighbors": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["neighbors"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v7/operations/resolve-symbol": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["resolveSymbol"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v7/operations/search": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["search"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v7/operations/stale": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["stale"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v7/operations/trace": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["trace"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v7/operations/verification-bindings": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["verificationBindings"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v7/operations/verification-runs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["verificationRuns"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        /** FailureEnvelope */
        CheckStatementFailureOutput: {
            error: components["schemas"]["CheckStatementFailureOutputOperationError"];
            /** @constant */
            operation?: "check-statement";
            /** @constant */
            protocol_version: 7;
        };
        /** @enum {string} */
        CheckStatementFailureOutputInvalidInputReason: "required" | "invalid_value" | "malformed_json" | "unknown_field" | "too_large";
        /** @description Keeps the handler's native error separate from preparation failure. */
        CheckStatementFailureOutputOperationError: components["schemas"]["CheckStatementFailureOutputOperationFailure"] | components["schemas"]["CheckStatementFailureOutputStatementFailure"];
        CheckStatementFailureOutputOperationFailure: {
            field: string | null;
            /** @constant */
            kind: "invalid_input";
            reason: components["schemas"]["CheckStatementFailureOutputInvalidInputReason"];
        } | {
            /** @constant */
            kind: "protocol_mismatch";
            /** Format: uint32 */
            requested: number;
            /** Format: uint32 */
            supported: number;
        } | {
            /** @constant */
            kind: "unknown_operation";
        } | {
            /** @constant */
            kind: "unauthenticated";
        } | {
            /** @constant */
            kind: "access_denied";
        } | {
            /** @constant */
            kind: "unknown_target";
        } | {
            /** @constant */
            kind: "unknown_scope";
        } | {
            /** @constant */
            kind: "unavailable_needs";
        } | {
            /** @constant */
            kind: "internal";
        };
        CheckStatementFailureOutputStatementFailure: never;
        /** DataFreeCall */
        CheckStatementRequestInput: {
            request: components["schemas"]["CheckStatementRequestInputCheckStatementRequest"];
        };
        /** @description The fixed request shape for one statement preflight. */
        CheckStatementRequestInputCheckStatementRequest: {
            statement: string;
        };
        /**
         * Report
         * @description A report from the fixed ASD-STE100 Issue 9 analyzer.
         */
        CheckStatementSuccessOutput: {
            analyzer_version: string;
            findings: components["schemas"]["CheckStatementSuccessOutputFinding"][];
            issue: components["schemas"]["CheckStatementSuccessOutputStandardIssue"];
            standard: components["schemas"]["CheckStatementSuccessOutputStandard"];
        };
        /** @description One nonconformance found in descriptive text. */
        CheckStatementSuccessOutputFinding: {
            kind: components["schemas"]["CheckStatementSuccessOutputFindingKind"];
            message: string;
            rule: components["schemas"]["CheckStatementSuccessOutputRuleNumber"];
            span: components["schemas"]["CheckStatementSuccessOutputSpan"];
        };
        /**
         * @description The disposition of a finding.
         * @enum {string}
         */
        CheckStatementSuccessOutputFindingKind: "violation";
        /**
         * @description An ASD-STE100 Issue 9 rule implemented by this analyzer.
         * @enum {string}
         */
        CheckStatementSuccessOutputRuleNumber: "1.1" | "4.2" | "6.3" | "6.6" | "8.1";
        /** @description A half-open UTF-8 byte range in the analyzed text. */
        CheckStatementSuccessOutputSpan: {
            /** Format: uint */
            end: number;
            /** Format: uint */
            start: number;
        };
        /**
         * @description The authority used by the analyzer.
         * @enum {string}
         */
        CheckStatementSuccessOutputStandard: "ASD-STE100";
        /**
         * Format: uint8
         * @description The fixed issue of the standard used by the analyzer.
         * @constant
         */
        CheckStatementSuccessOutputStandardIssue: 9;
        /** FailureEnvelope */
        EvidenceFailureOutput: {
            error: components["schemas"]["EvidenceFailureOutputOperationError"];
            /** @constant */
            operation?: "evidence";
            /** @constant */
            protocol_version: 7;
        };
        /** @enum {string} */
        EvidenceFailureOutputInvalidInputReason: "required" | "invalid_value" | "malformed_json" | "unknown_field" | "too_large";
        EvidenceFailureOutputMovedUnit: {
            live: string;
            stored: string;
            unit: string;
        };
        /** @description Keeps the handler's native error separate from preparation failure. */
        EvidenceFailureOutputOperationError: components["schemas"]["EvidenceFailureOutputOperationFailure"] | components["schemas"]["EvidenceFailureOutputReadFailure"];
        EvidenceFailureOutputOperationFailure: {
            field: string | null;
            /** @constant */
            kind: "invalid_input";
            reason: components["schemas"]["EvidenceFailureOutputInvalidInputReason"];
        } | {
            /** @constant */
            kind: "protocol_mismatch";
            /** Format: uint32 */
            requested: number;
            /** Format: uint32 */
            supported: number;
        } | {
            /** @constant */
            kind: "unknown_operation";
        } | {
            /** @constant */
            kind: "unauthenticated";
        } | {
            /** @constant */
            kind: "access_denied";
        } | {
            /** @constant */
            kind: "unknown_target";
        } | {
            /** @constant */
            kind: "unknown_scope";
        } | {
            /** @constant */
            kind: "unavailable_needs";
        } | {
            /** @constant */
            kind: "internal";
        };
        EvidenceFailureOutputReadFailure: {
            /** @constant */
            kind: "file_access_denied";
        } | {
            /** @constant */
            kind: "file_unavailable";
        } | {
            /** @constant */
            kind: "git_unavailable";
        } | {
            /** @constant */
            kind: "git_revision_not_found";
        } | {
            /** @constant */
            kind: "no_projection";
        } | {
            digest: string;
            instance_id: string;
            /** @constant */
            kind: "stale";
            moved: components["schemas"]["EvidenceFailureOutputMovedUnit"][];
            /** Format: int64 */
            serial: number;
        } | {
            /** @constant */
            kind: "unit_unreadable";
            unit: string;
        } | {
            /** @constant */
            kind: "schema_behind";
        } | {
            /** @constant */
            kind: "half_migrated";
        } | {
            /** @constant */
            kind: "read_failed";
        };
        /** RepositoryCall */
        EvidenceRequestInput: {
            context: components["schemas"]["EvidenceRequestInputRepositoryContext"];
            request: components["schemas"]["EvidenceRequestInputEvidenceQuery"];
        };
        /**
         * @description Read everything standing behind one Rule.
         *
         *     `base` is what makes the stale report computable: stale means the code
         *     carrying the evidence changed, and that is read from a diff. Without a
         *     base the response says so rather than guessing.
         */
        EvidenceRequestInputEvidenceQuery: {
            /** @default null */
            base?: string | null;
            /** @default null */
            head?: string | null;
            /** @default false */
            include_retired?: boolean;
            /**
             * Format: uint
             * @default 50
             */
            limit?: number;
            /**
             * Format: uint32
             * @default null
             */
            protocol_version?: number | null;
            rule: string;
        };
        /** @description Which freshness step a read runs before it answers. */
        EvidenceRequestInputFreshnessPolicy: "catch_up" | "annotate_only" | "refuse_stale";
        EvidenceRequestInputRepositoryContext: {
            /** @default null */
            freshness?: components["schemas"]["EvidenceRequestInputFreshnessPolicy"] | null;
            repository: string;
            scope: string;
        };
        /**
         * QueryResponse
         * @description The envelope every query primitive answers in.
         *
         *     The protocol version travels with the answer, so a caller holding a
         *     recorded response can tell which contract produced it, `operation`
         *     names which primitive it came from, and `stamp` says what the answer
         *     reflects.
         */
        EvidenceSuccessOutput: {
            freshness_cause?: components["schemas"]["EvidenceSuccessOutputFreshnessCause"] | null;
            freshness_error?: string | null;
            has_more: boolean;
            implementation_bindings: components["schemas"]["EvidenceSuccessOutputImplementationBinding"][];
            /**
             * @description The cut flag of each list; `has_more` is the OR of the four.
             * @default false
             */
            implementation_bindings_has_more: boolean;
            latest_verification_run?: components["schemas"]["EvidenceSuccessOutputVerificationRun"] | null;
            /** Format: uint */
            limit: number;
            /** @constant */
            operation: "evidence";
            /** @constant */
            protocol_version: 7;
            review_required: boolean;
            reviews: components["schemas"]["EvidenceSuccessOutputRequirementReview"][];
            /** @default false */
            reviews_has_more: boolean;
            rule_id: string;
            stale: components["schemas"]["EvidenceSuccessOutputStaleEvidence"] | null;
            stamp: components["schemas"]["EvidenceSuccessOutputStamp"];
            verification_bindings: components["schemas"]["EvidenceSuccessOutputVerificationBinding"][];
            /** @default false */
            verification_bindings_has_more: boolean;
            verification_runs: components["schemas"]["EvidenceSuccessOutputVerificationRun"][];
            /** @default false */
            verification_runs_has_more: boolean;
        };
        /** @description One graph-cited evidence site resolved against both ends of a diff. */
        EvidenceSuccessOutputEvidenceDiffSite: {
            /** Format: uint */
            end_line?: number | null;
            file_path: string;
            kind: components["schemas"]["EvidenceSuccessOutputEvidenceSiteKind"];
            /** Format: uint */
            line?: number | null;
            original_file_path?: string | null;
            /** Format: uint */
            original_line?: number | null;
            state: components["schemas"]["EvidenceSuccessOutputEvidenceDiffState"];
            subject_id: string;
        };
        /**
         * @description How one graph evidence path relates to a selected Git diff.
         * @enum {string}
         */
        EvidenceSuccessOutputEvidenceDiffState: "untouched" | "touched" | "moved" | "gone";
        /**
         * @description The graph relationship that makes a path evidence.
         * @enum {string}
         */
        EvidenceSuccessOutputEvidenceSiteKind: "rule_binding" | "verification" | "annotation" | "source_reference";
        /**
         * @description The stage that failed is known even when its lower-level error is not public.
         * @enum {string}
         */
        EvidenceSuccessOutputFreshnessCause: "catch_up_failed";
        /**
         * @description One canonical primary implementation relationship from an exported
         *     production symbol to a Rule.
         */
        EvidenceSuccessOutputImplementationBinding: {
            declared_by: string;
            file: string;
            id: components["schemas"]["EvidenceSuccessOutputStableId"];
            retired?: boolean;
            rule_id: components["schemas"]["EvidenceSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["EvidenceSuccessOutputScopeId"];
            symbol: string;
        };
        /**
         * @description One record of a Rule whose evidence needs review because the Requirement
         *     it serves was restated.
         *
         *     The record keeps why review was asked for. A verification run arriving
         *     after the change clears it in place rather than removing the reason.
         */
        EvidenceSuccessOutputRequirementReview: {
            after: string;
            before: string;
            /** Format: int64 */
            changed_at: number;
            /** Format: int64 */
            cleared_at?: number | null;
            cleared_by_run?: components["schemas"]["EvidenceSuccessOutputStableId"] | null;
            field: string;
            id: components["schemas"]["EvidenceSuccessOutputStableId"];
            requirement_id: components["schemas"]["EvidenceSuccessOutputStableId"];
            rule_id: components["schemas"]["EvidenceSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["EvidenceSuccessOutputScopeId"];
        };
        /**
         * @description A scope id. The inner `String` is private and `new` is the only way in, so
         *     every `ScopeId` in existence satisfies [`is_well_formed_id`].
         */
        EvidenceSuccessOutputScopeId: string;
        /**
         * @description A stable artifact id. The inner `String` is private and `new` is the only
         *     way in, so every `StableId` in existence satisfies [`is_well_formed_id`].
         */
        EvidenceSuccessOutputStableId: string;
        /** @description What a commit range did to the code carrying a Rule's evidence. */
        EvidenceSuccessOutputStaleEvidence: {
            base: string;
            head: string;
            sites: components["schemas"]["EvidenceSuccessOutputEvidenceDiffSite"][];
        };
        /**
         * @description What a query answer reflects: the projection revision the rows came
         *     from, the freshness step the reader ran, and which parts of the answer
         *     the revision covers.
         *
         *     `serial` and `digest` name the latest `projection_revision` row and
         *     `instance_id` the `projection_instance` row; serials compare only within
         *     one instance. `attested` names the projection tables behind the answer.
         *     `live` names what the stamp does not cover, from a closed list:
         *     `canonical` (canonical shards), `scanned_sites` (a working-tree scan),
         *     `verification_runs` (cache JSONL), and `diff` (git). A stamp never
         *     implies freshness for anything it does not list.
         */
        EvidenceSuccessOutputStamp: {
            attested: string[];
            /**
             * Format: uint32
             * @description The reader logic version. It moves when the reader answers
             *     differently over the same rows, never for a migration.
             */
            derivation: number;
            digest: string;
            instance_id: string;
            live: string[];
            policy: components["schemas"]["EvidenceSuccessOutputStampPolicy"];
            /** Format: int64 */
            serial: number;
        };
        /** @description The freshness step a read ran before it answered. */
        EvidenceSuccessOutputStampPolicy: "catch_up" | "annotate_only" | "refuse_stale" | "catch_up_failed";
        /** @description One durable language-authored relationship from a code site to a Rule. */
        EvidenceSuccessOutputVerificationBinding: {
            declared_by: string;
            file: string;
            id: components["schemas"]["EvidenceSuccessOutputStableId"];
            key: string;
            method: components["schemas"]["EvidenceSuccessOutputVerificationMethod"];
            retired?: boolean;
            rule_id: components["schemas"]["EvidenceSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["EvidenceSuccessOutputScopeId"];
            symbol?: string | null;
        };
        /**
         * @description How a verification binding supports its Rule.
         * @enum {string}
         */
        EvidenceSuccessOutputVerificationMethod: "exhaustion" | "property" | "examples" | "conformance" | "construction" | "proof";
        /**
         * @description Volatile evidence from one language-owned verification callback.
         *
         *     Runs live in Provenance's derived cache rather than canonical state: a
         *     local test run must not dirty the repository. `rule_id` is the join back
         *     to the canonical graph.
         */
        EvidenceSuccessOutputVerificationRun: {
            binding_id?: components["schemas"]["EvidenceSuccessOutputStableId"] | null;
            commit?: string | null;
            /** Format: int64 */
            completed_at?: number | null;
            declared_by: string;
            error?: string | null;
            file?: string | null;
            id: components["schemas"]["EvidenceSuccessOutputStableId"];
            method: string;
            rule_id: components["schemas"]["EvidenceSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["EvidenceSuccessOutputScopeId"];
            /** Format: int64 */
            started_at: number;
            status: components["schemas"]["EvidenceSuccessOutputVerificationRunStatus"];
            symbol?: string | null;
        };
        /**
         * @description The lifecycle state of one callback-backed verification run.
         * @enum {string}
         */
        EvidenceSuccessOutputVerificationRunStatus: "running" | "passed" | "failed";
        /** FailureEnvelope */
        GetFailureOutput: {
            error: components["schemas"]["GetFailureOutputOperationError"];
            /** @constant */
            operation?: "get";
            /** @constant */
            protocol_version: 7;
        };
        /** @enum {string} */
        GetFailureOutputInvalidInputReason: "required" | "invalid_value" | "malformed_json" | "unknown_field" | "too_large";
        GetFailureOutputMovedUnit: {
            live: string;
            stored: string;
            unit: string;
        };
        /** @description Keeps the handler's native error separate from preparation failure. */
        GetFailureOutputOperationError: components["schemas"]["GetFailureOutputOperationFailure"] | components["schemas"]["GetFailureOutputReadFailure"];
        GetFailureOutputOperationFailure: {
            field: string | null;
            /** @constant */
            kind: "invalid_input";
            reason: components["schemas"]["GetFailureOutputInvalidInputReason"];
        } | {
            /** @constant */
            kind: "protocol_mismatch";
            /** Format: uint32 */
            requested: number;
            /** Format: uint32 */
            supported: number;
        } | {
            /** @constant */
            kind: "unknown_operation";
        } | {
            /** @constant */
            kind: "unauthenticated";
        } | {
            /** @constant */
            kind: "access_denied";
        } | {
            /** @constant */
            kind: "unknown_target";
        } | {
            /** @constant */
            kind: "unknown_scope";
        } | {
            /** @constant */
            kind: "unavailable_needs";
        } | {
            /** @constant */
            kind: "internal";
        };
        GetFailureOutputReadFailure: {
            /** @constant */
            kind: "file_access_denied";
        } | {
            /** @constant */
            kind: "file_unavailable";
        } | {
            /** @constant */
            kind: "git_unavailable";
        } | {
            /** @constant */
            kind: "git_revision_not_found";
        } | {
            /** @constant */
            kind: "no_projection";
        } | {
            digest: string;
            instance_id: string;
            /** @constant */
            kind: "stale";
            moved: components["schemas"]["GetFailureOutputMovedUnit"][];
            /** Format: int64 */
            serial: number;
        } | {
            /** @constant */
            kind: "unit_unreadable";
            unit: string;
        } | {
            /** @constant */
            kind: "schema_behind";
        } | {
            /** @constant */
            kind: "half_migrated";
        } | {
            /** @constant */
            kind: "read_failed";
        };
        /** RepositoryCall */
        GetRequestInput: {
            context: components["schemas"]["GetRequestInputRepositoryContext"];
            request: components["schemas"]["GetRequestInputGetQuery"];
        };
        /** @description Which freshness step a read runs before it answers. */
        GetRequestInputFreshnessPolicy: "catch_up" | "annotate_only" | "refuse_stale";
        /** @description Fetch one record by canonical ID. */
        GetRequestInputGetQuery: {
            id: string;
            /** @default false */
            include_retired?: boolean;
            node_type: components["schemas"]["GetRequestInputNodeType"];
            /**
             * Format: uint32
             * @default null
             */
            protocol_version?: number | null;
        };
        /** @enum {string} */
        GetRequestInputNodeType: "source" | "requirement" | "resolution" | "rule" | "topic" | "question" | "domain" | "boundary";
        GetRequestInputRepositoryContext: {
            /** @default null */
            freshness?: components["schemas"]["GetRequestInputFreshnessPolicy"] | null;
            repository: string;
            scope: string;
        };
        /**
         * QueryResponse
         * @description The envelope every query primitive answers in.
         *
         *     The protocol version travels with the answer, so a caller holding a
         *     recorded response can tell which contract produced it, `operation`
         *     names which primitive it came from, and `stamp` says what the answer
         *     reflects.
         */
        GetSuccessOutput: {
            found: boolean;
            freshness_cause?: components["schemas"]["GetSuccessOutputFreshnessCause"] | null;
            freshness_error?: string | null;
            node?: components["schemas"]["GetSuccessOutputGraphNode"] | null;
            /** @constant */
            operation: "get";
            /** @constant */
            protocol_version: 7;
            stamp: components["schemas"]["GetSuccessOutputStamp"];
        };
        GetSuccessOutputArtifactLink: {
            target_id: components["schemas"]["GetSuccessOutputStableId"];
            target_type: components["schemas"]["GetSuccessOutputArtifactLinkTargetType"];
        };
        /** @enum {string} */
        GetSuccessOutputArtifactLinkTargetType: "source" | "requirement" | "resolution" | "rule";
        GetSuccessOutputBoundary: {
            id: components["schemas"]["GetSuccessOutputStableId"];
            requirement_id: components["schemas"]["GetSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["GetSuccessOutputScopeId"];
            source_ref?: components["schemas"]["GetSuccessOutputSourceReference"] | null;
            statement: string;
        };
        /** @description One owner-local path to a language-authored declaration. */
        GetSuccessOutputDeclarationAddress: string[];
        GetSuccessOutputDomain: {
            color?: string | null;
            description?: string | null;
            id: components["schemas"]["GetSuccessOutputStableId"];
            name: string;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["GetSuccessOutputScopeId"];
        };
        /**
         * @description The stage that failed is known even when its lower-level error is not public.
         * @enum {string}
         */
        GetSuccessOutputFreshnessCause: "catch_up_failed";
        /**
         * @description One canonical record as a query hands it back.
         *
         *     Each variant carries the record the store already writes, so a primitive
         *     never invents a second vocabulary for a Requirement or a Rule. The
         *     `node_type` tag is the same word a relation row uses for its endpoints.
         */
        GetSuccessOutputGraphNode: components["schemas"]["GetSuccessOutputSource"] | components["schemas"]["GetSuccessOutputRequirement"] | components["schemas"]["GetSuccessOutputResolution"] | components["schemas"]["GetSuccessOutputRule"] | components["schemas"]["GetSuccessOutputTopic"] | components["schemas"]["GetSuccessOutputQuestion"] | components["schemas"]["GetSuccessOutputDomain"] | components["schemas"]["GetSuccessOutputBoundary"];
        GetSuccessOutputQuestion: {
            answer?: string | null;
            /** Format: int64 */
            claimed_at?: number | null;
            claimed_by?: string | null;
            contradicts?: components["schemas"]["GetSuccessOutputStableId"] | null;
            id: components["schemas"]["GetSuccessOutputStableId"];
            /** @default [] */
            links: components["schemas"]["GetSuccessOutputArtifactLink"][];
            question: string;
            requirement_id: components["schemas"]["GetSuccessOutputStableId"];
            resolution_id?: components["schemas"]["GetSuccessOutputStableId"] | null;
            /** @description The verb that resolves this question, chosen when the question is minted. */
            resolution_method: components["schemas"]["GetSuccessOutputResolutionMethod"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["GetSuccessOutputScopeId"];
            status: components["schemas"]["GetSuccessOutputQuestionStatus"];
            topic_id: components["schemas"]["GetSuccessOutputStableId"];
        };
        /** @enum {string} */
        GetSuccessOutputQuestionStatus: "open" | "blocked_on_human" | "answered";
        GetSuccessOutputRequirement: {
            declaration_address?: components["schemas"]["GetSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            depends_on?: components["schemas"]["GetSuccessOutputStableId"][];
            description?: string | null;
            domain_id?: components["schemas"]["GetSuccessOutputStableId"] | null;
            /**
             * @description Deliberately unstructured free text: the dim view of decisions and
             *     investigations that are coming but cannot yet be phrased sharply.
             */
            fog?: string | null;
            id: components["schemas"]["GetSuccessOutputStableId"];
            origin_message?: components["schemas"]["GetSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["GetSuccessOutputStableId"] | null;
            refines?: components["schemas"]["GetSuccessOutputStableId"] | null;
            retired?: boolean;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["GetSuccessOutputScopeId"];
            source_refs?: components["schemas"]["GetSuccessOutputSourceReference"][];
            spawned_by?: components["schemas"]["GetSuccessOutputStableId"] | null;
            statement: string;
            status: components["schemas"]["GetSuccessOutputRequirementStatus"];
            supersedes?: components["schemas"]["GetSuccessOutputStableId"][];
        };
        /** @enum {string} */
        GetSuccessOutputRequirementStatus: "active" | "discovery" | "refinement" | "resolved";
        GetSuccessOutputResolution: {
            /** Format: int64 */
            approved_at?: number | null;
            approved_by?: string | null;
            /** Format: double */
            confidence?: number | null;
            context?: string | null;
            enforcement?: string | null;
            id: components["schemas"]["GetSuccessOutputStableId"];
            /** @default [] */
            inputs: components["schemas"]["GetSuccessOutputResolutionInput"][];
            made_by?: string | null;
            origin_message?: components["schemas"]["GetSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["GetSuccessOutputStableId"] | null;
            position: string;
            rationale: string;
            requirement_ids?: components["schemas"]["GetSuccessOutputStableId"][];
            review_on: string | null;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["GetSuccessOutputScopeId"];
            status: components["schemas"]["GetSuccessOutputResolutionStatus"];
            supersedes?: components["schemas"]["GetSuccessOutputStableId"][];
            title: string;
        };
        GetSuccessOutputResolutionInput: {
            input_type: components["schemas"]["GetSuccessOutputResolutionInputType"];
            reference: string;
            summary: string;
        };
        /** @enum {string} */
        GetSuccessOutputResolutionInputType: "regulatory" | "legal_advice" | "commercial" | "benchmark" | "technical" | "incident" | "source_material";
        /** @enum {string} */
        GetSuccessOutputResolutionMethod: "grill" | "prototype" | "research" | "verify" | "task";
        /** @enum {string} */
        GetSuccessOutputResolutionStatus: "draft" | "review" | "proposed" | "approved" | "rejected" | "revised" | "superseded" | "abandoned";
        GetSuccessOutputRule: {
            declaration_address?: components["schemas"]["GetSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            description?: string | null;
            id: components["schemas"]["GetSuccessOutputStableId"];
            name?: string | null;
            origin_message?: components["schemas"]["GetSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["GetSuccessOutputStableId"] | null;
            requirement_ids?: components["schemas"]["GetSuccessOutputStableId"][];
            resolution_ids?: components["schemas"]["GetSuccessOutputStableId"][];
            retired?: boolean;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["GetSuccessOutputScopeId"];
            severity: components["schemas"]["GetSuccessOutputRuleSeverity"];
            source_document?: string | null;
            source_section?: string | null;
            statement: string;
            status: components["schemas"]["GetSuccessOutputRuleStatus"];
        };
        /** @enum {string} */
        GetSuccessOutputRuleSeverity: "low" | "medium" | "high" | "critical";
        /** @enum {string} */
        GetSuccessOutputRuleStatus: "draft" | "review" | "active" | "deprecated" | "archived";
        /**
         * @description A scope id. The inner `String` is private and `new` is the only way in, so
         *     every `ScopeId` in existence satisfies [`is_well_formed_id`].
         */
        GetSuccessOutputScopeId: string;
        GetSuccessOutputSource: {
            commit_pin?: string | null;
            declaration_address?: components["schemas"]["GetSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            /** Format: int64 */
            effective_date?: number | null;
            id: components["schemas"]["GetSuccessOutputStableId"];
            name: string;
            origin_message?: components["schemas"]["GetSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["GetSuccessOutputStableId"] | null;
            reference?: string | null;
            retired?: boolean;
            /** Format: int64 */
            review_date?: number | null;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["GetSuccessOutputScopeId"];
            source_type: components["schemas"]["GetSuccessOutputSourceType"];
            supersedes?: components["schemas"]["GetSuccessOutputStableId"][];
            url: string | null;
        };
        GetSuccessOutputSourceReference: {
            clause?: string | null;
            source_id: components["schemas"]["GetSuccessOutputStableId"];
        };
        /** @enum {string} */
        GetSuccessOutputSourceType: "policy" | "document" | "legislation" | "company_agreement" | "system_state" | "external_integration" | "domain_knowledge" | "project_artifact" | "incident" | "api_spec";
        /**
         * @description A stable artifact id. The inner `String` is private and `new` is the only
         *     way in, so every `StableId` in existence satisfies [`is_well_formed_id`].
         */
        GetSuccessOutputStableId: string;
        /**
         * @description What a query answer reflects: the projection revision the rows came
         *     from, the freshness step the reader ran, and which parts of the answer
         *     the revision covers.
         *
         *     `serial` and `digest` name the latest `projection_revision` row and
         *     `instance_id` the `projection_instance` row; serials compare only within
         *     one instance. `attested` names the projection tables behind the answer.
         *     `live` names what the stamp does not cover, from a closed list:
         *     `canonical` (canonical shards), `scanned_sites` (a working-tree scan),
         *     `verification_runs` (cache JSONL), and `diff` (git). A stamp never
         *     implies freshness for anything it does not list.
         */
        GetSuccessOutputStamp: {
            attested: string[];
            /**
             * Format: uint32
             * @description The reader logic version. It moves when the reader answers
             *     differently over the same rows, never for a migration.
             */
            derivation: number;
            digest: string;
            instance_id: string;
            live: string[];
            policy: components["schemas"]["GetSuccessOutputStampPolicy"];
            /** Format: int64 */
            serial: number;
        };
        /** @description The freshness step a read ran before it answered. */
        GetSuccessOutputStampPolicy: "catch_up" | "annotate_only" | "refuse_stale" | "catch_up_failed";
        GetSuccessOutputTopic: {
            /** Format: int64 */
            claimed_at?: number | null;
            claimed_by?: string | null;
            id: components["schemas"]["GetSuccessOutputStableId"];
            /** @default [] */
            links: components["schemas"]["GetSuccessOutputArtifactLink"][];
            requirement_id: components["schemas"]["GetSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["GetSuccessOutputScopeId"];
            status: components["schemas"]["GetSuccessOutputTopicStatus"];
            title: string;
        };
        /** @enum {string} */
        GetSuccessOutputTopicStatus: "open" | "explored" | "closed";
        /** FailureEnvelope */
        ImpactFailureOutput: {
            error: components["schemas"]["ImpactFailureOutputOperationError"];
            /** @constant */
            operation?: "impact";
            /** @constant */
            protocol_version: 7;
        };
        /** @enum {string} */
        ImpactFailureOutputInvalidInputReason: "required" | "invalid_value" | "malformed_json" | "unknown_field" | "too_large";
        ImpactFailureOutputMovedUnit: {
            live: string;
            stored: string;
            unit: string;
        };
        /** @description Keeps the handler's native error separate from preparation failure. */
        ImpactFailureOutputOperationError: components["schemas"]["ImpactFailureOutputOperationFailure"] | components["schemas"]["ImpactFailureOutputReadFailure"];
        ImpactFailureOutputOperationFailure: {
            field: string | null;
            /** @constant */
            kind: "invalid_input";
            reason: components["schemas"]["ImpactFailureOutputInvalidInputReason"];
        } | {
            /** @constant */
            kind: "protocol_mismatch";
            /** Format: uint32 */
            requested: number;
            /** Format: uint32 */
            supported: number;
        } | {
            /** @constant */
            kind: "unknown_operation";
        } | {
            /** @constant */
            kind: "unauthenticated";
        } | {
            /** @constant */
            kind: "access_denied";
        } | {
            /** @constant */
            kind: "unknown_target";
        } | {
            /** @constant */
            kind: "unknown_scope";
        } | {
            /** @constant */
            kind: "unavailable_needs";
        } | {
            /** @constant */
            kind: "internal";
        };
        ImpactFailureOutputReadFailure: {
            /** @constant */
            kind: "file_access_denied";
        } | {
            /** @constant */
            kind: "file_unavailable";
        } | {
            /** @constant */
            kind: "git_unavailable";
        } | {
            /** @constant */
            kind: "git_revision_not_found";
        } | {
            /** @constant */
            kind: "no_projection";
        } | {
            digest: string;
            instance_id: string;
            /** @constant */
            kind: "stale";
            moved: components["schemas"]["ImpactFailureOutputMovedUnit"][];
            /** Format: int64 */
            serial: number;
        } | {
            /** @constant */
            kind: "unit_unreadable";
            unit: string;
        } | {
            /** @constant */
            kind: "schema_behind";
        } | {
            /** @constant */
            kind: "half_migrated";
        } | {
            /** @constant */
            kind: "read_failed";
        };
        /** RepositoryCall */
        ImpactRequestInput: {
            context: components["schemas"]["ImpactRequestInputRepositoryContext"];
            request: components["schemas"]["ImpactRequestInputImpactQuery"];
        };
        /** @description Which freshness step a read runs before it answers. */
        ImpactRequestInputFreshnessPolicy: "catch_up" | "annotate_only" | "refuse_stale";
        /** @description Read the Rules a record reaches, with the code standing behind them. */
        ImpactRequestInputImpactQuery: {
            id: string;
            /** @default false */
            include_retired?: boolean;
            /**
             * Format: uint
             * @default 50
             */
            limit?: number;
            /** @default null */
            node_type?: components["schemas"]["ImpactRequestInputNodeType"] | null;
            /**
             * Format: uint32
             * @default null
             */
            protocol_version?: number | null;
        };
        /** @enum {string} */
        ImpactRequestInputNodeType: "source" | "requirement" | "resolution" | "rule" | "topic" | "question" | "domain" | "boundary";
        ImpactRequestInputRepositoryContext: {
            /** @default null */
            freshness?: components["schemas"]["ImpactRequestInputFreshnessPolicy"] | null;
            repository: string;
            scope: string;
        };
        /**
         * QueryResponse
         * @description The envelope every query primitive answers in.
         *
         *     The protocol version travels with the answer, so a caller holding a
         *     recorded response can tell which contract produced it, `operation`
         *     names which primitive it came from, and `stamp` says what the answer
         *     reflects.
         */
        ImpactSuccessOutput: {
            affected_rules: components["schemas"]["ImpactSuccessOutputAffectedRule"][];
            freshness_cause?: components["schemas"]["ImpactSuccessOutputFreshnessCause"] | null;
            freshness_error?: string | null;
            has_more: boolean;
            id: string;
            /** Format: uint */
            limit: number;
            /** @constant */
            operation: "impact";
            /** @constant */
            protocol_version: 7;
            /**
             * @description The working-tree scan stopped at the configured file count, so the
             *     scanned sites are a lower bound.
             * @default false
             */
            scan_cut: boolean;
            stamp: components["schemas"]["ImpactSuccessOutputStamp"];
        };
        /** @description One Rule a change reaches, with the code that stands behind it. */
        ImpactSuccessOutputAffectedRule: {
            id: components["schemas"]["ImpactSuccessOutputStableId"];
            implementations: components["schemas"]["ImpactSuccessOutputImplementationSite"][];
            verifications: components["schemas"]["ImpactSuccessOutputVerificationSite"][];
        };
        /**
         * @description The stage that failed is known even when its lower-level error is not public.
         * @enum {string}
         */
        ImpactSuccessOutputFreshnessCause: "catch_up_failed";
        /** @description Where a Rule is implemented. */
        ImpactSuccessOutputImplementationSite: {
            file: string;
            /** Format: uint */
            line?: number | null;
            symbol?: string | null;
        };
        /**
         * @description A stable artifact id. The inner `String` is private and `new` is the only
         *     way in, so every `StableId` in existence satisfies [`is_well_formed_id`].
         */
        ImpactSuccessOutputStableId: string;
        /**
         * @description What a query answer reflects: the projection revision the rows came
         *     from, the freshness step the reader ran, and which parts of the answer
         *     the revision covers.
         *
         *     `serial` and `digest` name the latest `projection_revision` row and
         *     `instance_id` the `projection_instance` row; serials compare only within
         *     one instance. `attested` names the projection tables behind the answer.
         *     `live` names what the stamp does not cover, from a closed list:
         *     `canonical` (canonical shards), `scanned_sites` (a working-tree scan),
         *     `verification_runs` (cache JSONL), and `diff` (git). A stamp never
         *     implies freshness for anything it does not list.
         */
        ImpactSuccessOutputStamp: {
            attested: string[];
            /**
             * Format: uint32
             * @description The reader logic version. It moves when the reader answers
             *     differently over the same rows, never for a migration.
             */
            derivation: number;
            digest: string;
            instance_id: string;
            live: string[];
            policy: components["schemas"]["ImpactSuccessOutputStampPolicy"];
            /** Format: int64 */
            serial: number;
        };
        /** @description The freshness step a read ran before it answered. */
        ImpactSuccessOutputStampPolicy: "catch_up" | "annotate_only" | "refuse_stale" | "catch_up_failed";
        /** @description Where a Rule is verified, and how. */
        ImpactSuccessOutputVerificationSite: {
            declared_by?: string | null;
            file: string;
            key?: string | null;
            /** Format: uint */
            line?: number | null;
            method: string;
            symbol?: string | null;
        };
        /** FailureEnvelope */
        InfoFailureOutput: {
            error: components["schemas"]["InfoFailureOutputOperationError"];
            /** @constant */
            operation?: "info";
            /** @constant */
            protocol_version: 7;
        };
        /** @enum {string} */
        InfoFailureOutputInvalidInputReason: "required" | "invalid_value" | "malformed_json" | "unknown_field" | "too_large";
        InfoFailureOutputMovedUnit: {
            live: string;
            stored: string;
            unit: string;
        };
        /** @description Keeps the handler's native error separate from preparation failure. */
        InfoFailureOutputOperationError: components["schemas"]["InfoFailureOutputOperationFailure"] | components["schemas"]["InfoFailureOutputReadFailure"];
        InfoFailureOutputOperationFailure: {
            field: string | null;
            /** @constant */
            kind: "invalid_input";
            reason: components["schemas"]["InfoFailureOutputInvalidInputReason"];
        } | {
            /** @constant */
            kind: "protocol_mismatch";
            /** Format: uint32 */
            requested: number;
            /** Format: uint32 */
            supported: number;
        } | {
            /** @constant */
            kind: "unknown_operation";
        } | {
            /** @constant */
            kind: "unauthenticated";
        } | {
            /** @constant */
            kind: "access_denied";
        } | {
            /** @constant */
            kind: "unknown_target";
        } | {
            /** @constant */
            kind: "unknown_scope";
        } | {
            /** @constant */
            kind: "unavailable_needs";
        } | {
            /** @constant */
            kind: "internal";
        };
        InfoFailureOutputReadFailure: {
            /** @constant */
            kind: "file_access_denied";
        } | {
            /** @constant */
            kind: "file_unavailable";
        } | {
            /** @constant */
            kind: "git_unavailable";
        } | {
            /** @constant */
            kind: "git_revision_not_found";
        } | {
            /** @constant */
            kind: "no_projection";
        } | {
            digest: string;
            instance_id: string;
            /** @constant */
            kind: "stale";
            moved: components["schemas"]["InfoFailureOutputMovedUnit"][];
            /** Format: int64 */
            serial: number;
        } | {
            /** @constant */
            kind: "unit_unreadable";
            unit: string;
        } | {
            /** @constant */
            kind: "schema_behind";
        } | {
            /** @constant */
            kind: "half_migrated";
        } | {
            /** @constant */
            kind: "read_failed";
        };
        /** RepositoryCall */
        InfoRequestInput: {
            context: components["schemas"]["InfoRequestInputRepositoryTarget"];
            request: components["schemas"]["InfoRequestInputInfoRequest"];
        };
        InfoRequestInputInfoRequest: Record<string, never>;
        InfoRequestInputRepositoryTarget: {
            repository: string;
        };
        /**
         * RepositoryInfo
         * @description External metadata names the requested target, never a host path.
         */
        InfoSuccessOutput: {
            engine_version: string;
            /** @constant */
            protocol_version: 7;
            repository: string;
            /** Format: uint32 */
            state_schema_version: number;
        };
        /** HostMetadata */
        MetadataOutput: {
            engine_version: string;
            /** @constant */
            protocol_version: 7;
        };
        /** FailureEnvelope */
        NeighborsFailureOutput: {
            error: components["schemas"]["NeighborsFailureOutputOperationError"];
            /** @constant */
            operation?: "neighbors";
            /** @constant */
            protocol_version: 7;
        };
        /** @enum {string} */
        NeighborsFailureOutputInvalidInputReason: "required" | "invalid_value" | "malformed_json" | "unknown_field" | "too_large";
        NeighborsFailureOutputMovedUnit: {
            live: string;
            stored: string;
            unit: string;
        };
        /** @description Keeps the handler's native error separate from preparation failure. */
        NeighborsFailureOutputOperationError: components["schemas"]["NeighborsFailureOutputOperationFailure"] | components["schemas"]["NeighborsFailureOutputReadFailure"];
        NeighborsFailureOutputOperationFailure: {
            field: string | null;
            /** @constant */
            kind: "invalid_input";
            reason: components["schemas"]["NeighborsFailureOutputInvalidInputReason"];
        } | {
            /** @constant */
            kind: "protocol_mismatch";
            /** Format: uint32 */
            requested: number;
            /** Format: uint32 */
            supported: number;
        } | {
            /** @constant */
            kind: "unknown_operation";
        } | {
            /** @constant */
            kind: "unauthenticated";
        } | {
            /** @constant */
            kind: "access_denied";
        } | {
            /** @constant */
            kind: "unknown_target";
        } | {
            /** @constant */
            kind: "unknown_scope";
        } | {
            /** @constant */
            kind: "unavailable_needs";
        } | {
            /** @constant */
            kind: "internal";
        };
        NeighborsFailureOutputReadFailure: {
            /** @constant */
            kind: "file_access_denied";
        } | {
            /** @constant */
            kind: "file_unavailable";
        } | {
            /** @constant */
            kind: "git_unavailable";
        } | {
            /** @constant */
            kind: "git_revision_not_found";
        } | {
            /** @constant */
            kind: "no_projection";
        } | {
            digest: string;
            instance_id: string;
            /** @constant */
            kind: "stale";
            moved: components["schemas"]["NeighborsFailureOutputMovedUnit"][];
            /** Format: int64 */
            serial: number;
        } | {
            /** @constant */
            kind: "unit_unreadable";
            unit: string;
        } | {
            /** @constant */
            kind: "schema_behind";
        } | {
            /** @constant */
            kind: "half_migrated";
        } | {
            /** @constant */
            kind: "read_failed";
        };
        /** RepositoryCall */
        NeighborsRequestInput: {
            context: components["schemas"]["NeighborsRequestInputRepositoryContext"];
            request: components["schemas"]["NeighborsRequestInputNeighborsQuery"];
        };
        /**
         * @description Which way a query follows a relation.
         *
         *     `out` reads the relations the named record holds in its own fields, `in`
         *     reads the relations other records hold toward it, and `both` reads every
         *     relation from either end.
         * @enum {string}
         */
        NeighborsRequestInputDirection: "out" | "in" | "both";
        /** @description Which freshness step a read runs before it answers. */
        NeighborsRequestInputFreshnessPolicy: "catch_up" | "annotate_only" | "refuse_stale";
        /** @description Read the records one hop from a record. */
        NeighborsRequestInputNeighborsQuery: {
            /** @default both */
            direction?: components["schemas"]["NeighborsRequestInputDirection"];
            id: string;
            /** @default false */
            include_retired?: boolean;
            /**
             * Format: uint
             * @default 50
             */
            limit?: number;
            /** @default null */
            node_type?: components["schemas"]["NeighborsRequestInputNodeType"] | null;
            /**
             * Format: uint32
             * @default null
             */
            protocol_version?: number | null;
            /** @default [] */
            relations?: string[];
        };
        /** @enum {string} */
        NeighborsRequestInputNodeType: "source" | "requirement" | "resolution" | "rule" | "topic" | "question" | "domain" | "boundary";
        NeighborsRequestInputRepositoryContext: {
            /** @default null */
            freshness?: components["schemas"]["NeighborsRequestInputFreshnessPolicy"] | null;
            repository: string;
            scope: string;
        };
        /**
         * QueryResponse
         * @description The envelope every query primitive answers in.
         *
         *     The protocol version travels with the answer, so a caller holding a
         *     recorded response can tell which contract produced it, `operation`
         *     names which primitive it came from, and `stamp` says what the answer
         *     reflects.
         */
        NeighborsSuccessOutput: {
            freshness_cause?: components["schemas"]["NeighborsSuccessOutputFreshnessCause"] | null;
            freshness_error?: string | null;
            has_more: boolean;
            id: string;
            /** Format: uint */
            limit: number;
            neighbors: components["schemas"]["NeighborsSuccessOutputNeighbor"][];
            /** @constant */
            operation: "neighbors";
            /** @constant */
            protocol_version: 7;
            stamp: components["schemas"]["NeighborsSuccessOutputStamp"];
        };
        NeighborsSuccessOutputArtifactLink: {
            target_id: components["schemas"]["NeighborsSuccessOutputStableId"];
            target_type: components["schemas"]["NeighborsSuccessOutputArtifactLinkTargetType"];
        };
        /** @enum {string} */
        NeighborsSuccessOutputArtifactLinkTargetType: "source" | "requirement" | "resolution" | "rule";
        NeighborsSuccessOutputBoundary: {
            id: components["schemas"]["NeighborsSuccessOutputStableId"];
            requirement_id: components["schemas"]["NeighborsSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["NeighborsSuccessOutputScopeId"];
            source_ref?: components["schemas"]["NeighborsSuccessOutputSourceReference"] | null;
            statement: string;
        };
        /** @description One owner-local path to a language-authored declaration. */
        NeighborsSuccessOutputDeclarationAddress: string[];
        /**
         * @description Which way a query follows a relation.
         *
         *     `out` reads the relations the named record holds in its own fields, `in`
         *     reads the relations other records hold toward it, and `both` reads every
         *     relation from either end.
         * @enum {string}
         */
        NeighborsSuccessOutputDirection: "out" | "in" | "both";
        NeighborsSuccessOutputDomain: {
            color?: string | null;
            description?: string | null;
            id: components["schemas"]["NeighborsSuccessOutputStableId"];
            name: string;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["NeighborsSuccessOutputScopeId"];
        };
        /**
         * @description The stage that failed is known even when its lower-level error is not public.
         * @enum {string}
         */
        NeighborsSuccessOutputFreshnessCause: "catch_up_failed";
        /**
         * @description One canonical record as a query hands it back.
         *
         *     Each variant carries the record the store already writes, so a primitive
         *     never invents a second vocabulary for a Requirement or a Rule. The
         *     `node_type` tag is the same word a relation row uses for its endpoints.
         */
        NeighborsSuccessOutputGraphNode: components["schemas"]["NeighborsSuccessOutputSource"] | components["schemas"]["NeighborsSuccessOutputRequirement"] | components["schemas"]["NeighborsSuccessOutputResolution"] | components["schemas"]["NeighborsSuccessOutputRule"] | components["schemas"]["NeighborsSuccessOutputTopic"] | components["schemas"]["NeighborsSuccessOutputQuestion"] | components["schemas"]["NeighborsSuccessOutputDomain"] | components["schemas"]["NeighborsSuccessOutputBoundary"];
        /** @description One record reached in a single hop, with the relation that reached it. */
        NeighborsSuccessOutputNeighbor: {
            direction: components["schemas"]["NeighborsSuccessOutputDirection"];
            node: components["schemas"]["NeighborsSuccessOutputGraphNode"];
            relation: string;
        };
        NeighborsSuccessOutputQuestion: {
            answer?: string | null;
            /** Format: int64 */
            claimed_at?: number | null;
            claimed_by?: string | null;
            contradicts?: components["schemas"]["NeighborsSuccessOutputStableId"] | null;
            id: components["schemas"]["NeighborsSuccessOutputStableId"];
            /** @default [] */
            links: components["schemas"]["NeighborsSuccessOutputArtifactLink"][];
            question: string;
            requirement_id: components["schemas"]["NeighborsSuccessOutputStableId"];
            resolution_id?: components["schemas"]["NeighborsSuccessOutputStableId"] | null;
            /** @description The verb that resolves this question, chosen when the question is minted. */
            resolution_method: components["schemas"]["NeighborsSuccessOutputResolutionMethod"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["NeighborsSuccessOutputScopeId"];
            status: components["schemas"]["NeighborsSuccessOutputQuestionStatus"];
            topic_id: components["schemas"]["NeighborsSuccessOutputStableId"];
        };
        /** @enum {string} */
        NeighborsSuccessOutputQuestionStatus: "open" | "blocked_on_human" | "answered";
        NeighborsSuccessOutputRequirement: {
            declaration_address?: components["schemas"]["NeighborsSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            depends_on?: components["schemas"]["NeighborsSuccessOutputStableId"][];
            description?: string | null;
            domain_id?: components["schemas"]["NeighborsSuccessOutputStableId"] | null;
            /**
             * @description Deliberately unstructured free text: the dim view of decisions and
             *     investigations that are coming but cannot yet be phrased sharply.
             */
            fog?: string | null;
            id: components["schemas"]["NeighborsSuccessOutputStableId"];
            origin_message?: components["schemas"]["NeighborsSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["NeighborsSuccessOutputStableId"] | null;
            refines?: components["schemas"]["NeighborsSuccessOutputStableId"] | null;
            retired?: boolean;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["NeighborsSuccessOutputScopeId"];
            source_refs?: components["schemas"]["NeighborsSuccessOutputSourceReference"][];
            spawned_by?: components["schemas"]["NeighborsSuccessOutputStableId"] | null;
            statement: string;
            status: components["schemas"]["NeighborsSuccessOutputRequirementStatus"];
            supersedes?: components["schemas"]["NeighborsSuccessOutputStableId"][];
        };
        /** @enum {string} */
        NeighborsSuccessOutputRequirementStatus: "active" | "discovery" | "refinement" | "resolved";
        NeighborsSuccessOutputResolution: {
            /** Format: int64 */
            approved_at?: number | null;
            approved_by?: string | null;
            /** Format: double */
            confidence?: number | null;
            context?: string | null;
            enforcement?: string | null;
            id: components["schemas"]["NeighborsSuccessOutputStableId"];
            /** @default [] */
            inputs: components["schemas"]["NeighborsSuccessOutputResolutionInput"][];
            made_by?: string | null;
            origin_message?: components["schemas"]["NeighborsSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["NeighborsSuccessOutputStableId"] | null;
            position: string;
            rationale: string;
            requirement_ids?: components["schemas"]["NeighborsSuccessOutputStableId"][];
            review_on: string | null;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["NeighborsSuccessOutputScopeId"];
            status: components["schemas"]["NeighborsSuccessOutputResolutionStatus"];
            supersedes?: components["schemas"]["NeighborsSuccessOutputStableId"][];
            title: string;
        };
        NeighborsSuccessOutputResolutionInput: {
            input_type: components["schemas"]["NeighborsSuccessOutputResolutionInputType"];
            reference: string;
            summary: string;
        };
        /** @enum {string} */
        NeighborsSuccessOutputResolutionInputType: "regulatory" | "legal_advice" | "commercial" | "benchmark" | "technical" | "incident" | "source_material";
        /** @enum {string} */
        NeighborsSuccessOutputResolutionMethod: "grill" | "prototype" | "research" | "verify" | "task";
        /** @enum {string} */
        NeighborsSuccessOutputResolutionStatus: "draft" | "review" | "proposed" | "approved" | "rejected" | "revised" | "superseded" | "abandoned";
        NeighborsSuccessOutputRule: {
            declaration_address?: components["schemas"]["NeighborsSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            description?: string | null;
            id: components["schemas"]["NeighborsSuccessOutputStableId"];
            name?: string | null;
            origin_message?: components["schemas"]["NeighborsSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["NeighborsSuccessOutputStableId"] | null;
            requirement_ids?: components["schemas"]["NeighborsSuccessOutputStableId"][];
            resolution_ids?: components["schemas"]["NeighborsSuccessOutputStableId"][];
            retired?: boolean;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["NeighborsSuccessOutputScopeId"];
            severity: components["schemas"]["NeighborsSuccessOutputRuleSeverity"];
            source_document?: string | null;
            source_section?: string | null;
            statement: string;
            status: components["schemas"]["NeighborsSuccessOutputRuleStatus"];
        };
        /** @enum {string} */
        NeighborsSuccessOutputRuleSeverity: "low" | "medium" | "high" | "critical";
        /** @enum {string} */
        NeighborsSuccessOutputRuleStatus: "draft" | "review" | "active" | "deprecated" | "archived";
        /**
         * @description A scope id. The inner `String` is private and `new` is the only way in, so
         *     every `ScopeId` in existence satisfies [`is_well_formed_id`].
         */
        NeighborsSuccessOutputScopeId: string;
        NeighborsSuccessOutputSource: {
            commit_pin?: string | null;
            declaration_address?: components["schemas"]["NeighborsSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            /** Format: int64 */
            effective_date?: number | null;
            id: components["schemas"]["NeighborsSuccessOutputStableId"];
            name: string;
            origin_message?: components["schemas"]["NeighborsSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["NeighborsSuccessOutputStableId"] | null;
            reference?: string | null;
            retired?: boolean;
            /** Format: int64 */
            review_date?: number | null;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["NeighborsSuccessOutputScopeId"];
            source_type: components["schemas"]["NeighborsSuccessOutputSourceType"];
            supersedes?: components["schemas"]["NeighborsSuccessOutputStableId"][];
            url: string | null;
        };
        NeighborsSuccessOutputSourceReference: {
            clause?: string | null;
            source_id: components["schemas"]["NeighborsSuccessOutputStableId"];
        };
        /** @enum {string} */
        NeighborsSuccessOutputSourceType: "policy" | "document" | "legislation" | "company_agreement" | "system_state" | "external_integration" | "domain_knowledge" | "project_artifact" | "incident" | "api_spec";
        /**
         * @description A stable artifact id. The inner `String` is private and `new` is the only
         *     way in, so every `StableId` in existence satisfies [`is_well_formed_id`].
         */
        NeighborsSuccessOutputStableId: string;
        /**
         * @description What a query answer reflects: the projection revision the rows came
         *     from, the freshness step the reader ran, and which parts of the answer
         *     the revision covers.
         *
         *     `serial` and `digest` name the latest `projection_revision` row and
         *     `instance_id` the `projection_instance` row; serials compare only within
         *     one instance. `attested` names the projection tables behind the answer.
         *     `live` names what the stamp does not cover, from a closed list:
         *     `canonical` (canonical shards), `scanned_sites` (a working-tree scan),
         *     `verification_runs` (cache JSONL), and `diff` (git). A stamp never
         *     implies freshness for anything it does not list.
         */
        NeighborsSuccessOutputStamp: {
            attested: string[];
            /**
             * Format: uint32
             * @description The reader logic version. It moves when the reader answers
             *     differently over the same rows, never for a migration.
             */
            derivation: number;
            digest: string;
            instance_id: string;
            live: string[];
            policy: components["schemas"]["NeighborsSuccessOutputStampPolicy"];
            /** Format: int64 */
            serial: number;
        };
        /** @description The freshness step a read ran before it answered. */
        NeighborsSuccessOutputStampPolicy: "catch_up" | "annotate_only" | "refuse_stale" | "catch_up_failed";
        NeighborsSuccessOutputTopic: {
            /** Format: int64 */
            claimed_at?: number | null;
            claimed_by?: string | null;
            id: components["schemas"]["NeighborsSuccessOutputStableId"];
            /** @default [] */
            links: components["schemas"]["NeighborsSuccessOutputArtifactLink"][];
            requirement_id: components["schemas"]["NeighborsSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["NeighborsSuccessOutputScopeId"];
            status: components["schemas"]["NeighborsSuccessOutputTopicStatus"];
            title: string;
        };
        /** @enum {string} */
        NeighborsSuccessOutputTopicStatus: "open" | "explored" | "closed";
        /** FailureEnvelope */
        ResolveSymbolFailureOutput: {
            error: components["schemas"]["ResolveSymbolFailureOutputOperationError"];
            /** @constant */
            operation?: "resolve-symbol";
            /** @constant */
            protocol_version: 7;
        };
        /** @enum {string} */
        ResolveSymbolFailureOutputInvalidInputReason: "required" | "invalid_value" | "malformed_json" | "unknown_field" | "too_large";
        ResolveSymbolFailureOutputMovedUnit: {
            live: string;
            stored: string;
            unit: string;
        };
        /** @description Keeps the handler's native error separate from preparation failure. */
        ResolveSymbolFailureOutputOperationError: components["schemas"]["ResolveSymbolFailureOutputOperationFailure"] | components["schemas"]["ResolveSymbolFailureOutputReadFailure"];
        ResolveSymbolFailureOutputOperationFailure: {
            field: string | null;
            /** @constant */
            kind: "invalid_input";
            reason: components["schemas"]["ResolveSymbolFailureOutputInvalidInputReason"];
        } | {
            /** @constant */
            kind: "protocol_mismatch";
            /** Format: uint32 */
            requested: number;
            /** Format: uint32 */
            supported: number;
        } | {
            /** @constant */
            kind: "unknown_operation";
        } | {
            /** @constant */
            kind: "unauthenticated";
        } | {
            /** @constant */
            kind: "access_denied";
        } | {
            /** @constant */
            kind: "unknown_target";
        } | {
            /** @constant */
            kind: "unknown_scope";
        } | {
            /** @constant */
            kind: "unavailable_needs";
        } | {
            /** @constant */
            kind: "internal";
        };
        ResolveSymbolFailureOutputReadFailure: {
            /** @constant */
            kind: "file_access_denied";
        } | {
            /** @constant */
            kind: "file_unavailable";
        } | {
            /** @constant */
            kind: "git_unavailable";
        } | {
            /** @constant */
            kind: "git_revision_not_found";
        } | {
            /** @constant */
            kind: "no_projection";
        } | {
            digest: string;
            instance_id: string;
            /** @constant */
            kind: "stale";
            moved: components["schemas"]["ResolveSymbolFailureOutputMovedUnit"][];
            /** Format: int64 */
            serial: number;
        } | {
            /** @constant */
            kind: "unit_unreadable";
            unit: string;
        } | {
            /** @constant */
            kind: "schema_behind";
        } | {
            /** @constant */
            kind: "half_migrated";
        } | {
            /** @constant */
            kind: "read_failed";
        };
        /** RepositoryCall */
        ResolveSymbolRequestInput: {
            context: components["schemas"]["ResolveSymbolRequestInputRepositoryContext"];
            request: components["schemas"]["ResolveSymbolRequestInputResolveSymbolQuery"];
        };
        /** @description Which freshness step a read runs before it answers. */
        ResolveSymbolRequestInputFreshnessPolicy: "catch_up" | "annotate_only" | "refuse_stale";
        ResolveSymbolRequestInputRepositoryContext: {
            /** @default null */
            freshness?: components["schemas"]["ResolveSymbolRequestInputFreshnessPolicy"] | null;
            repository: string;
            scope: string;
        };
        /** @description Read the Rules bound to one code site. */
        ResolveSymbolRequestInputResolveSymbolQuery: {
            file: string;
            /** @default false */
            include_retired?: boolean;
            /**
             * Format: uint
             * @default 50
             */
            limit?: number;
            /**
             * Format: uint
             * @default null
             */
            line?: number | null;
            /**
             * Format: uint32
             * @default null
             */
            protocol_version?: number | null;
            /** @default null */
            symbol?: string | null;
        };
        /**
         * QueryResponse
         * @description The envelope every query primitive answers in.
         *
         *     The protocol version travels with the answer, so a caller holding a
         *     recorded response can tell which contract produced it, `operation`
         *     names which primitive it came from, and `stamp` says what the answer
         *     reflects.
         */
        ResolveSymbolSuccessOutput: {
            file: string;
            freshness_cause?: components["schemas"]["ResolveSymbolSuccessOutputFreshnessCause"] | null;
            freshness_error?: string | null;
            has_more: boolean;
            /** Format: uint */
            limit: number;
            /** @constant */
            operation: "resolve-symbol";
            /** @constant */
            protocol_version: 7;
            rules: components["schemas"]["ResolveSymbolSuccessOutputGraphNode"][];
            stamp: components["schemas"]["ResolveSymbolSuccessOutputStamp"];
            symbol?: string | null;
        };
        ResolveSymbolSuccessOutputArtifactLink: {
            target_id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
            target_type: components["schemas"]["ResolveSymbolSuccessOutputArtifactLinkTargetType"];
        };
        /** @enum {string} */
        ResolveSymbolSuccessOutputArtifactLinkTargetType: "source" | "requirement" | "resolution" | "rule";
        ResolveSymbolSuccessOutputBoundary: {
            id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
            requirement_id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["ResolveSymbolSuccessOutputScopeId"];
            source_ref?: components["schemas"]["ResolveSymbolSuccessOutputSourceReference"] | null;
            statement: string;
        };
        /** @description One owner-local path to a language-authored declaration. */
        ResolveSymbolSuccessOutputDeclarationAddress: string[];
        ResolveSymbolSuccessOutputDomain: {
            color?: string | null;
            description?: string | null;
            id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
            name: string;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["ResolveSymbolSuccessOutputScopeId"];
        };
        /**
         * @description The stage that failed is known even when its lower-level error is not public.
         * @enum {string}
         */
        ResolveSymbolSuccessOutputFreshnessCause: "catch_up_failed";
        /**
         * @description One canonical record as a query hands it back.
         *
         *     Each variant carries the record the store already writes, so a primitive
         *     never invents a second vocabulary for a Requirement or a Rule. The
         *     `node_type` tag is the same word a relation row uses for its endpoints.
         */
        ResolveSymbolSuccessOutputGraphNode: components["schemas"]["ResolveSymbolSuccessOutputSource"] | components["schemas"]["ResolveSymbolSuccessOutputRequirement"] | components["schemas"]["ResolveSymbolSuccessOutputResolution"] | components["schemas"]["ResolveSymbolSuccessOutputRule"] | components["schemas"]["ResolveSymbolSuccessOutputTopic"] | components["schemas"]["ResolveSymbolSuccessOutputQuestion"] | components["schemas"]["ResolveSymbolSuccessOutputDomain"] | components["schemas"]["ResolveSymbolSuccessOutputBoundary"];
        ResolveSymbolSuccessOutputQuestion: {
            answer?: string | null;
            /** Format: int64 */
            claimed_at?: number | null;
            claimed_by?: string | null;
            contradicts?: components["schemas"]["ResolveSymbolSuccessOutputStableId"] | null;
            id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
            /** @default [] */
            links: components["schemas"]["ResolveSymbolSuccessOutputArtifactLink"][];
            question: string;
            requirement_id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
            resolution_id?: components["schemas"]["ResolveSymbolSuccessOutputStableId"] | null;
            /** @description The verb that resolves this question, chosen when the question is minted. */
            resolution_method: components["schemas"]["ResolveSymbolSuccessOutputResolutionMethod"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["ResolveSymbolSuccessOutputScopeId"];
            status: components["schemas"]["ResolveSymbolSuccessOutputQuestionStatus"];
            topic_id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
        };
        /** @enum {string} */
        ResolveSymbolSuccessOutputQuestionStatus: "open" | "blocked_on_human" | "answered";
        ResolveSymbolSuccessOutputRequirement: {
            declaration_address?: components["schemas"]["ResolveSymbolSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            depends_on?: components["schemas"]["ResolveSymbolSuccessOutputStableId"][];
            description?: string | null;
            domain_id?: components["schemas"]["ResolveSymbolSuccessOutputStableId"] | null;
            /**
             * @description Deliberately unstructured free text: the dim view of decisions and
             *     investigations that are coming but cannot yet be phrased sharply.
             */
            fog?: string | null;
            id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
            origin_message?: components["schemas"]["ResolveSymbolSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["ResolveSymbolSuccessOutputStableId"] | null;
            refines?: components["schemas"]["ResolveSymbolSuccessOutputStableId"] | null;
            retired?: boolean;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["ResolveSymbolSuccessOutputScopeId"];
            source_refs?: components["schemas"]["ResolveSymbolSuccessOutputSourceReference"][];
            spawned_by?: components["schemas"]["ResolveSymbolSuccessOutputStableId"] | null;
            statement: string;
            status: components["schemas"]["ResolveSymbolSuccessOutputRequirementStatus"];
            supersedes?: components["schemas"]["ResolveSymbolSuccessOutputStableId"][];
        };
        /** @enum {string} */
        ResolveSymbolSuccessOutputRequirementStatus: "active" | "discovery" | "refinement" | "resolved";
        ResolveSymbolSuccessOutputResolution: {
            /** Format: int64 */
            approved_at?: number | null;
            approved_by?: string | null;
            /** Format: double */
            confidence?: number | null;
            context?: string | null;
            enforcement?: string | null;
            id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
            /** @default [] */
            inputs: components["schemas"]["ResolveSymbolSuccessOutputResolutionInput"][];
            made_by?: string | null;
            origin_message?: components["schemas"]["ResolveSymbolSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["ResolveSymbolSuccessOutputStableId"] | null;
            position: string;
            rationale: string;
            requirement_ids?: components["schemas"]["ResolveSymbolSuccessOutputStableId"][];
            review_on: string | null;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["ResolveSymbolSuccessOutputScopeId"];
            status: components["schemas"]["ResolveSymbolSuccessOutputResolutionStatus"];
            supersedes?: components["schemas"]["ResolveSymbolSuccessOutputStableId"][];
            title: string;
        };
        ResolveSymbolSuccessOutputResolutionInput: {
            input_type: components["schemas"]["ResolveSymbolSuccessOutputResolutionInputType"];
            reference: string;
            summary: string;
        };
        /** @enum {string} */
        ResolveSymbolSuccessOutputResolutionInputType: "regulatory" | "legal_advice" | "commercial" | "benchmark" | "technical" | "incident" | "source_material";
        /** @enum {string} */
        ResolveSymbolSuccessOutputResolutionMethod: "grill" | "prototype" | "research" | "verify" | "task";
        /** @enum {string} */
        ResolveSymbolSuccessOutputResolutionStatus: "draft" | "review" | "proposed" | "approved" | "rejected" | "revised" | "superseded" | "abandoned";
        ResolveSymbolSuccessOutputRule: {
            declaration_address?: components["schemas"]["ResolveSymbolSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            description?: string | null;
            id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
            name?: string | null;
            origin_message?: components["schemas"]["ResolveSymbolSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["ResolveSymbolSuccessOutputStableId"] | null;
            requirement_ids?: components["schemas"]["ResolveSymbolSuccessOutputStableId"][];
            resolution_ids?: components["schemas"]["ResolveSymbolSuccessOutputStableId"][];
            retired?: boolean;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["ResolveSymbolSuccessOutputScopeId"];
            severity: components["schemas"]["ResolveSymbolSuccessOutputRuleSeverity"];
            source_document?: string | null;
            source_section?: string | null;
            statement: string;
            status: components["schemas"]["ResolveSymbolSuccessOutputRuleStatus"];
        };
        /** @enum {string} */
        ResolveSymbolSuccessOutputRuleSeverity: "low" | "medium" | "high" | "critical";
        /** @enum {string} */
        ResolveSymbolSuccessOutputRuleStatus: "draft" | "review" | "active" | "deprecated" | "archived";
        /**
         * @description A scope id. The inner `String` is private and `new` is the only way in, so
         *     every `ScopeId` in existence satisfies [`is_well_formed_id`].
         */
        ResolveSymbolSuccessOutputScopeId: string;
        ResolveSymbolSuccessOutputSource: {
            commit_pin?: string | null;
            declaration_address?: components["schemas"]["ResolveSymbolSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            /** Format: int64 */
            effective_date?: number | null;
            id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
            name: string;
            origin_message?: components["schemas"]["ResolveSymbolSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["ResolveSymbolSuccessOutputStableId"] | null;
            reference?: string | null;
            retired?: boolean;
            /** Format: int64 */
            review_date?: number | null;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["ResolveSymbolSuccessOutputScopeId"];
            source_type: components["schemas"]["ResolveSymbolSuccessOutputSourceType"];
            supersedes?: components["schemas"]["ResolveSymbolSuccessOutputStableId"][];
            url: string | null;
        };
        ResolveSymbolSuccessOutputSourceReference: {
            clause?: string | null;
            source_id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
        };
        /** @enum {string} */
        ResolveSymbolSuccessOutputSourceType: "policy" | "document" | "legislation" | "company_agreement" | "system_state" | "external_integration" | "domain_knowledge" | "project_artifact" | "incident" | "api_spec";
        /**
         * @description A stable artifact id. The inner `String` is private and `new` is the only
         *     way in, so every `StableId` in existence satisfies [`is_well_formed_id`].
         */
        ResolveSymbolSuccessOutputStableId: string;
        /**
         * @description What a query answer reflects: the projection revision the rows came
         *     from, the freshness step the reader ran, and which parts of the answer
         *     the revision covers.
         *
         *     `serial` and `digest` name the latest `projection_revision` row and
         *     `instance_id` the `projection_instance` row; serials compare only within
         *     one instance. `attested` names the projection tables behind the answer.
         *     `live` names what the stamp does not cover, from a closed list:
         *     `canonical` (canonical shards), `scanned_sites` (a working-tree scan),
         *     `verification_runs` (cache JSONL), and `diff` (git). A stamp never
         *     implies freshness for anything it does not list.
         */
        ResolveSymbolSuccessOutputStamp: {
            attested: string[];
            /**
             * Format: uint32
             * @description The reader logic version. It moves when the reader answers
             *     differently over the same rows, never for a migration.
             */
            derivation: number;
            digest: string;
            instance_id: string;
            live: string[];
            policy: components["schemas"]["ResolveSymbolSuccessOutputStampPolicy"];
            /** Format: int64 */
            serial: number;
        };
        /** @description The freshness step a read ran before it answered. */
        ResolveSymbolSuccessOutputStampPolicy: "catch_up" | "annotate_only" | "refuse_stale" | "catch_up_failed";
        ResolveSymbolSuccessOutputTopic: {
            /** Format: int64 */
            claimed_at?: number | null;
            claimed_by?: string | null;
            id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
            /** @default [] */
            links: components["schemas"]["ResolveSymbolSuccessOutputArtifactLink"][];
            requirement_id: components["schemas"]["ResolveSymbolSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["ResolveSymbolSuccessOutputScopeId"];
            status: components["schemas"]["ResolveSymbolSuccessOutputTopicStatus"];
            title: string;
        };
        /** @enum {string} */
        ResolveSymbolSuccessOutputTopicStatus: "open" | "explored" | "closed";
        /** FailureEnvelope */
        SearchFailureOutput: {
            error: components["schemas"]["SearchFailureOutputOperationError"];
            /** @constant */
            operation?: "search";
            /** @constant */
            protocol_version: 7;
        };
        /** @enum {string} */
        SearchFailureOutputInvalidInputReason: "required" | "invalid_value" | "malformed_json" | "unknown_field" | "too_large";
        SearchFailureOutputMovedUnit: {
            live: string;
            stored: string;
            unit: string;
        };
        /** @description Keeps the handler's native error separate from preparation failure. */
        SearchFailureOutputOperationError: components["schemas"]["SearchFailureOutputOperationFailure"] | components["schemas"]["SearchFailureOutputReadFailure"];
        SearchFailureOutputOperationFailure: {
            field: string | null;
            /** @constant */
            kind: "invalid_input";
            reason: components["schemas"]["SearchFailureOutputInvalidInputReason"];
        } | {
            /** @constant */
            kind: "protocol_mismatch";
            /** Format: uint32 */
            requested: number;
            /** Format: uint32 */
            supported: number;
        } | {
            /** @constant */
            kind: "unknown_operation";
        } | {
            /** @constant */
            kind: "unauthenticated";
        } | {
            /** @constant */
            kind: "access_denied";
        } | {
            /** @constant */
            kind: "unknown_target";
        } | {
            /** @constant */
            kind: "unknown_scope";
        } | {
            /** @constant */
            kind: "unavailable_needs";
        } | {
            /** @constant */
            kind: "internal";
        };
        SearchFailureOutputReadFailure: {
            /** @constant */
            kind: "file_access_denied";
        } | {
            /** @constant */
            kind: "file_unavailable";
        } | {
            /** @constant */
            kind: "git_unavailable";
        } | {
            /** @constant */
            kind: "git_revision_not_found";
        } | {
            /** @constant */
            kind: "no_projection";
        } | {
            digest: string;
            instance_id: string;
            /** @constant */
            kind: "stale";
            moved: components["schemas"]["SearchFailureOutputMovedUnit"][];
            /** Format: int64 */
            serial: number;
        } | {
            /** @constant */
            kind: "unit_unreadable";
            unit: string;
        } | {
            /** @constant */
            kind: "schema_behind";
        } | {
            /** @constant */
            kind: "half_migrated";
        } | {
            /** @constant */
            kind: "read_failed";
        };
        /** RepositoryCall */
        SearchRequestInput: {
            context: components["schemas"]["SearchRequestInputRepositoryContext"];
            request: components["schemas"]["SearchRequestInputSearchQuery"];
        };
        /** @description Which freshness step a read runs before it answers. */
        SearchRequestInputFreshnessPolicy: "catch_up" | "annotate_only" | "refuse_stale";
        /** @enum {string} */
        SearchRequestInputNodeType: "source" | "requirement" | "resolution" | "rule" | "topic" | "question" | "domain" | "boundary";
        SearchRequestInputRepositoryContext: {
            /** @default null */
            freshness?: components["schemas"]["SearchRequestInputFreshnessPolicy"] | null;
            repository: string;
            scope: string;
        };
        /** @description Find records whose text contains a phrase. */
        SearchRequestInputSearchQuery: {
            /** @default false */
            include_retired?: boolean;
            /**
             * Format: uint
             * @default 50
             */
            limit?: number;
            /** @default [] */
            node_types?: components["schemas"]["SearchRequestInputNodeType"][];
            /**
             * Format: uint32
             * @default null
             */
            protocol_version?: number | null;
            text: string;
        };
        /**
         * QueryResponse
         * @description The envelope every query primitive answers in.
         *
         *     The protocol version travels with the answer, so a caller holding a
         *     recorded response can tell which contract produced it, `operation`
         *     names which primitive it came from, and `stamp` says what the answer
         *     reflects.
         */
        SearchSuccessOutput: {
            freshness_cause?: components["schemas"]["SearchSuccessOutputFreshnessCause"] | null;
            freshness_error?: string | null;
            has_more: boolean;
            /** Format: uint */
            limit: number;
            nodes: components["schemas"]["SearchSuccessOutputGraphNode"][];
            /** @constant */
            operation: "search";
            /** @constant */
            protocol_version: 7;
            stamp: components["schemas"]["SearchSuccessOutputStamp"];
        };
        SearchSuccessOutputArtifactLink: {
            target_id: components["schemas"]["SearchSuccessOutputStableId"];
            target_type: components["schemas"]["SearchSuccessOutputArtifactLinkTargetType"];
        };
        /** @enum {string} */
        SearchSuccessOutputArtifactLinkTargetType: "source" | "requirement" | "resolution" | "rule";
        SearchSuccessOutputBoundary: {
            id: components["schemas"]["SearchSuccessOutputStableId"];
            requirement_id: components["schemas"]["SearchSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["SearchSuccessOutputScopeId"];
            source_ref?: components["schemas"]["SearchSuccessOutputSourceReference"] | null;
            statement: string;
        };
        /** @description One owner-local path to a language-authored declaration. */
        SearchSuccessOutputDeclarationAddress: string[];
        SearchSuccessOutputDomain: {
            color?: string | null;
            description?: string | null;
            id: components["schemas"]["SearchSuccessOutputStableId"];
            name: string;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["SearchSuccessOutputScopeId"];
        };
        /**
         * @description The stage that failed is known even when its lower-level error is not public.
         * @enum {string}
         */
        SearchSuccessOutputFreshnessCause: "catch_up_failed";
        /**
         * @description One canonical record as a query hands it back.
         *
         *     Each variant carries the record the store already writes, so a primitive
         *     never invents a second vocabulary for a Requirement or a Rule. The
         *     `node_type` tag is the same word a relation row uses for its endpoints.
         */
        SearchSuccessOutputGraphNode: components["schemas"]["SearchSuccessOutputSource"] | components["schemas"]["SearchSuccessOutputRequirement"] | components["schemas"]["SearchSuccessOutputResolution"] | components["schemas"]["SearchSuccessOutputRule"] | components["schemas"]["SearchSuccessOutputTopic"] | components["schemas"]["SearchSuccessOutputQuestion"] | components["schemas"]["SearchSuccessOutputDomain"] | components["schemas"]["SearchSuccessOutputBoundary"];
        SearchSuccessOutputQuestion: {
            answer?: string | null;
            /** Format: int64 */
            claimed_at?: number | null;
            claimed_by?: string | null;
            contradicts?: components["schemas"]["SearchSuccessOutputStableId"] | null;
            id: components["schemas"]["SearchSuccessOutputStableId"];
            /** @default [] */
            links: components["schemas"]["SearchSuccessOutputArtifactLink"][];
            question: string;
            requirement_id: components["schemas"]["SearchSuccessOutputStableId"];
            resolution_id?: components["schemas"]["SearchSuccessOutputStableId"] | null;
            /** @description The verb that resolves this question, chosen when the question is minted. */
            resolution_method: components["schemas"]["SearchSuccessOutputResolutionMethod"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["SearchSuccessOutputScopeId"];
            status: components["schemas"]["SearchSuccessOutputQuestionStatus"];
            topic_id: components["schemas"]["SearchSuccessOutputStableId"];
        };
        /** @enum {string} */
        SearchSuccessOutputQuestionStatus: "open" | "blocked_on_human" | "answered";
        SearchSuccessOutputRequirement: {
            declaration_address?: components["schemas"]["SearchSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            depends_on?: components["schemas"]["SearchSuccessOutputStableId"][];
            description?: string | null;
            domain_id?: components["schemas"]["SearchSuccessOutputStableId"] | null;
            /**
             * @description Deliberately unstructured free text: the dim view of decisions and
             *     investigations that are coming but cannot yet be phrased sharply.
             */
            fog?: string | null;
            id: components["schemas"]["SearchSuccessOutputStableId"];
            origin_message?: components["schemas"]["SearchSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["SearchSuccessOutputStableId"] | null;
            refines?: components["schemas"]["SearchSuccessOutputStableId"] | null;
            retired?: boolean;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["SearchSuccessOutputScopeId"];
            source_refs?: components["schemas"]["SearchSuccessOutputSourceReference"][];
            spawned_by?: components["schemas"]["SearchSuccessOutputStableId"] | null;
            statement: string;
            status: components["schemas"]["SearchSuccessOutputRequirementStatus"];
            supersedes?: components["schemas"]["SearchSuccessOutputStableId"][];
        };
        /** @enum {string} */
        SearchSuccessOutputRequirementStatus: "active" | "discovery" | "refinement" | "resolved";
        SearchSuccessOutputResolution: {
            /** Format: int64 */
            approved_at?: number | null;
            approved_by?: string | null;
            /** Format: double */
            confidence?: number | null;
            context?: string | null;
            enforcement?: string | null;
            id: components["schemas"]["SearchSuccessOutputStableId"];
            /** @default [] */
            inputs: components["schemas"]["SearchSuccessOutputResolutionInput"][];
            made_by?: string | null;
            origin_message?: components["schemas"]["SearchSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["SearchSuccessOutputStableId"] | null;
            position: string;
            rationale: string;
            requirement_ids?: components["schemas"]["SearchSuccessOutputStableId"][];
            review_on: string | null;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["SearchSuccessOutputScopeId"];
            status: components["schemas"]["SearchSuccessOutputResolutionStatus"];
            supersedes?: components["schemas"]["SearchSuccessOutputStableId"][];
            title: string;
        };
        SearchSuccessOutputResolutionInput: {
            input_type: components["schemas"]["SearchSuccessOutputResolutionInputType"];
            reference: string;
            summary: string;
        };
        /** @enum {string} */
        SearchSuccessOutputResolutionInputType: "regulatory" | "legal_advice" | "commercial" | "benchmark" | "technical" | "incident" | "source_material";
        /** @enum {string} */
        SearchSuccessOutputResolutionMethod: "grill" | "prototype" | "research" | "verify" | "task";
        /** @enum {string} */
        SearchSuccessOutputResolutionStatus: "draft" | "review" | "proposed" | "approved" | "rejected" | "revised" | "superseded" | "abandoned";
        SearchSuccessOutputRule: {
            declaration_address?: components["schemas"]["SearchSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            description?: string | null;
            id: components["schemas"]["SearchSuccessOutputStableId"];
            name?: string | null;
            origin_message?: components["schemas"]["SearchSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["SearchSuccessOutputStableId"] | null;
            requirement_ids?: components["schemas"]["SearchSuccessOutputStableId"][];
            resolution_ids?: components["schemas"]["SearchSuccessOutputStableId"][];
            retired?: boolean;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["SearchSuccessOutputScopeId"];
            severity: components["schemas"]["SearchSuccessOutputRuleSeverity"];
            source_document?: string | null;
            source_section?: string | null;
            statement: string;
            status: components["schemas"]["SearchSuccessOutputRuleStatus"];
        };
        /** @enum {string} */
        SearchSuccessOutputRuleSeverity: "low" | "medium" | "high" | "critical";
        /** @enum {string} */
        SearchSuccessOutputRuleStatus: "draft" | "review" | "active" | "deprecated" | "archived";
        /**
         * @description A scope id. The inner `String` is private and `new` is the only way in, so
         *     every `ScopeId` in existence satisfies [`is_well_formed_id`].
         */
        SearchSuccessOutputScopeId: string;
        SearchSuccessOutputSource: {
            commit_pin?: string | null;
            declaration_address?: components["schemas"]["SearchSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            /** Format: int64 */
            effective_date?: number | null;
            id: components["schemas"]["SearchSuccessOutputStableId"];
            name: string;
            origin_message?: components["schemas"]["SearchSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["SearchSuccessOutputStableId"] | null;
            reference?: string | null;
            retired?: boolean;
            /** Format: int64 */
            review_date?: number | null;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["SearchSuccessOutputScopeId"];
            source_type: components["schemas"]["SearchSuccessOutputSourceType"];
            supersedes?: components["schemas"]["SearchSuccessOutputStableId"][];
            url: string | null;
        };
        SearchSuccessOutputSourceReference: {
            clause?: string | null;
            source_id: components["schemas"]["SearchSuccessOutputStableId"];
        };
        /** @enum {string} */
        SearchSuccessOutputSourceType: "policy" | "document" | "legislation" | "company_agreement" | "system_state" | "external_integration" | "domain_knowledge" | "project_artifact" | "incident" | "api_spec";
        /**
         * @description A stable artifact id. The inner `String` is private and `new` is the only
         *     way in, so every `StableId` in existence satisfies [`is_well_formed_id`].
         */
        SearchSuccessOutputStableId: string;
        /**
         * @description What a query answer reflects: the projection revision the rows came
         *     from, the freshness step the reader ran, and which parts of the answer
         *     the revision covers.
         *
         *     `serial` and `digest` name the latest `projection_revision` row and
         *     `instance_id` the `projection_instance` row; serials compare only within
         *     one instance. `attested` names the projection tables behind the answer.
         *     `live` names what the stamp does not cover, from a closed list:
         *     `canonical` (canonical shards), `scanned_sites` (a working-tree scan),
         *     `verification_runs` (cache JSONL), and `diff` (git). A stamp never
         *     implies freshness for anything it does not list.
         */
        SearchSuccessOutputStamp: {
            attested: string[];
            /**
             * Format: uint32
             * @description The reader logic version. It moves when the reader answers
             *     differently over the same rows, never for a migration.
             */
            derivation: number;
            digest: string;
            instance_id: string;
            live: string[];
            policy: components["schemas"]["SearchSuccessOutputStampPolicy"];
            /** Format: int64 */
            serial: number;
        };
        /** @description The freshness step a read ran before it answered. */
        SearchSuccessOutputStampPolicy: "catch_up" | "annotate_only" | "refuse_stale" | "catch_up_failed";
        SearchSuccessOutputTopic: {
            /** Format: int64 */
            claimed_at?: number | null;
            claimed_by?: string | null;
            id: components["schemas"]["SearchSuccessOutputStableId"];
            /** @default [] */
            links: components["schemas"]["SearchSuccessOutputArtifactLink"][];
            requirement_id: components["schemas"]["SearchSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["SearchSuccessOutputScopeId"];
            status: components["schemas"]["SearchSuccessOutputTopicStatus"];
            title: string;
        };
        /** @enum {string} */
        SearchSuccessOutputTopicStatus: "open" | "explored" | "closed";
        /** FailureEnvelope */
        StaleFailureOutput: {
            error: components["schemas"]["StaleFailureOutputOperationError"];
            /** @constant */
            operation?: "stale";
            /** @constant */
            protocol_version: 7;
        };
        /** @enum {string} */
        StaleFailureOutputInvalidInputReason: "required" | "invalid_value" | "malformed_json" | "unknown_field" | "too_large";
        StaleFailureOutputMovedUnit: {
            live: string;
            stored: string;
            unit: string;
        };
        /** @description Keeps the handler's native error separate from preparation failure. */
        StaleFailureOutputOperationError: components["schemas"]["StaleFailureOutputOperationFailure"] | components["schemas"]["StaleFailureOutputReadFailure"];
        StaleFailureOutputOperationFailure: {
            field: string | null;
            /** @constant */
            kind: "invalid_input";
            reason: components["schemas"]["StaleFailureOutputInvalidInputReason"];
        } | {
            /** @constant */
            kind: "protocol_mismatch";
            /** Format: uint32 */
            requested: number;
            /** Format: uint32 */
            supported: number;
        } | {
            /** @constant */
            kind: "unknown_operation";
        } | {
            /** @constant */
            kind: "unauthenticated";
        } | {
            /** @constant */
            kind: "access_denied";
        } | {
            /** @constant */
            kind: "unknown_target";
        } | {
            /** @constant */
            kind: "unknown_scope";
        } | {
            /** @constant */
            kind: "unavailable_needs";
        } | {
            /** @constant */
            kind: "internal";
        };
        StaleFailureOutputReadFailure: {
            /** @constant */
            kind: "file_access_denied";
        } | {
            /** @constant */
            kind: "file_unavailable";
        } | {
            /** @constant */
            kind: "git_unavailable";
        } | {
            /** @constant */
            kind: "git_revision_not_found";
        } | {
            /** @constant */
            kind: "no_projection";
        } | {
            digest: string;
            instance_id: string;
            /** @constant */
            kind: "stale";
            moved: components["schemas"]["StaleFailureOutputMovedUnit"][];
            /** Format: int64 */
            serial: number;
        } | {
            /** @constant */
            kind: "unit_unreadable";
            unit: string;
        } | {
            /** @constant */
            kind: "schema_behind";
        } | {
            /** @constant */
            kind: "half_migrated";
        } | {
            /** @constant */
            kind: "read_failed";
        };
        /** RepositoryCall */
        StaleRequestInput: {
            context: components["schemas"]["StaleRequestInputRepositoryContext"];
            request: components["schemas"]["StaleRequestInputStaleQuery"];
        };
        /** @description Which freshness step a read runs before it answers. */
        StaleRequestInputFreshnessPolicy: "catch_up" | "annotate_only" | "refuse_stale";
        StaleRequestInputRepositoryContext: {
            /** @default null */
            freshness?: components["schemas"]["StaleRequestInputFreshnessPolicy"] | null;
            repository: string;
            scope: string;
        };
        /** @description Read which evidence sites a commit range disturbed. */
        StaleRequestInputStaleQuery: {
            base: string;
            /** @default null */
            head?: string | null;
            /** @default false */
            include_retired?: boolean;
            /**
             * Format: uint
             * @default 50
             */
            limit?: number;
            /**
             * Format: uint32
             * @default null
             */
            protocol_version?: number | null;
            /** @default [] */
            rules?: string[];
        };
        /**
         * QueryResponse
         * @description The envelope every query primitive answers in.
         *
         *     The protocol version travels with the answer, so a caller holding a
         *     recorded response can tell which contract produced it, `operation`
         *     names which primitive it came from, and `stamp` says what the answer
         *     reflects.
         */
        StaleSuccessOutput: {
            base: string;
            /** Format: uint */
            files_changed: number;
            freshness_cause?: components["schemas"]["StaleSuccessOutputFreshnessCause"] | null;
            freshness_error?: string | null;
            has_more: boolean;
            head: string;
            /** Format: uint */
            limit: number;
            /** @constant */
            operation: "stale";
            /** @constant */
            protocol_version: 7;
            sites: components["schemas"]["StaleSuccessOutputEvidenceDiffSite"][];
            stamp: components["schemas"]["StaleSuccessOutputStamp"];
            summary: components["schemas"]["StaleSuccessOutputEvidenceDiffSummary"];
        };
        /** @description One graph-cited evidence site resolved against both ends of a diff. */
        StaleSuccessOutputEvidenceDiffSite: {
            /** Format: uint */
            end_line?: number | null;
            file_path: string;
            kind: components["schemas"]["StaleSuccessOutputEvidenceSiteKind"];
            /** Format: uint */
            line?: number | null;
            original_file_path?: string | null;
            /** Format: uint */
            original_line?: number | null;
            state: components["schemas"]["StaleSuccessOutputEvidenceDiffState"];
            subject_id: string;
        };
        /**
         * @description How one graph evidence path relates to a selected Git diff.
         * @enum {string}
         */
        StaleSuccessOutputEvidenceDiffState: "untouched" | "touched" | "moved" | "gone";
        StaleSuccessOutputEvidenceDiffSummary: {
            /** Format: uint */
            gone: number;
            /** Format: uint */
            moved: number;
            /** Format: uint */
            total_sites: number;
            /** Format: uint */
            touched: number;
            /** Format: uint */
            untouched: number;
        };
        /**
         * @description The graph relationship that makes a path evidence.
         * @enum {string}
         */
        StaleSuccessOutputEvidenceSiteKind: "rule_binding" | "verification" | "annotation" | "source_reference";
        /**
         * @description The stage that failed is known even when its lower-level error is not public.
         * @enum {string}
         */
        StaleSuccessOutputFreshnessCause: "catch_up_failed";
        /**
         * @description What a query answer reflects: the projection revision the rows came
         *     from, the freshness step the reader ran, and which parts of the answer
         *     the revision covers.
         *
         *     `serial` and `digest` name the latest `projection_revision` row and
         *     `instance_id` the `projection_instance` row; serials compare only within
         *     one instance. `attested` names the projection tables behind the answer.
         *     `live` names what the stamp does not cover, from a closed list:
         *     `canonical` (canonical shards), `scanned_sites` (a working-tree scan),
         *     `verification_runs` (cache JSONL), and `diff` (git). A stamp never
         *     implies freshness for anything it does not list.
         */
        StaleSuccessOutputStamp: {
            attested: string[];
            /**
             * Format: uint32
             * @description The reader logic version. It moves when the reader answers
             *     differently over the same rows, never for a migration.
             */
            derivation: number;
            digest: string;
            instance_id: string;
            live: string[];
            policy: components["schemas"]["StaleSuccessOutputStampPolicy"];
            /** Format: int64 */
            serial: number;
        };
        /** @description The freshness step a read ran before it answered. */
        StaleSuccessOutputStampPolicy: "catch_up" | "annotate_only" | "refuse_stale" | "catch_up_failed";
        /** FailureEnvelope */
        TraceFailureOutput: {
            error: components["schemas"]["TraceFailureOutputOperationError"];
            /** @constant */
            operation?: "trace";
            /** @constant */
            protocol_version: 7;
        };
        /** @enum {string} */
        TraceFailureOutputInvalidInputReason: "required" | "invalid_value" | "malformed_json" | "unknown_field" | "too_large";
        TraceFailureOutputMovedUnit: {
            live: string;
            stored: string;
            unit: string;
        };
        /** @description Keeps the handler's native error separate from preparation failure. */
        TraceFailureOutputOperationError: components["schemas"]["TraceFailureOutputOperationFailure"] | components["schemas"]["TraceFailureOutputReadFailure"];
        TraceFailureOutputOperationFailure: {
            field: string | null;
            /** @constant */
            kind: "invalid_input";
            reason: components["schemas"]["TraceFailureOutputInvalidInputReason"];
        } | {
            /** @constant */
            kind: "protocol_mismatch";
            /** Format: uint32 */
            requested: number;
            /** Format: uint32 */
            supported: number;
        } | {
            /** @constant */
            kind: "unknown_operation";
        } | {
            /** @constant */
            kind: "unauthenticated";
        } | {
            /** @constant */
            kind: "access_denied";
        } | {
            /** @constant */
            kind: "unknown_target";
        } | {
            /** @constant */
            kind: "unknown_scope";
        } | {
            /** @constant */
            kind: "unavailable_needs";
        } | {
            /** @constant */
            kind: "internal";
        };
        TraceFailureOutputReadFailure: {
            /** @constant */
            kind: "file_access_denied";
        } | {
            /** @constant */
            kind: "file_unavailable";
        } | {
            /** @constant */
            kind: "git_unavailable";
        } | {
            /** @constant */
            kind: "git_revision_not_found";
        } | {
            /** @constant */
            kind: "no_projection";
        } | {
            digest: string;
            instance_id: string;
            /** @constant */
            kind: "stale";
            moved: components["schemas"]["TraceFailureOutputMovedUnit"][];
            /** Format: int64 */
            serial: number;
        } | {
            /** @constant */
            kind: "unit_unreadable";
            unit: string;
        } | {
            /** @constant */
            kind: "schema_behind";
        } | {
            /** @constant */
            kind: "half_migrated";
        } | {
            /** @constant */
            kind: "read_failed";
        };
        /** RepositoryCall */
        TraceRequestInput: {
            context: components["schemas"]["TraceRequestInputRepositoryContext"];
            request: components["schemas"]["TraceRequestInputTraceQuery"];
        };
        /**
         * @description Which way a query follows a relation.
         *
         *     `out` reads the relations the named record holds in its own fields, `in`
         *     reads the relations other records hold toward it, and `both` reads every
         *     relation from either end.
         * @enum {string}
         */
        TraceRequestInputDirection: "out" | "in" | "both";
        /** @description Which freshness step a read runs before it answers. */
        TraceRequestInputFreshnessPolicy: "catch_up" | "annotate_only" | "refuse_stale";
        /** @enum {string} */
        TraceRequestInputNodeType: "source" | "requirement" | "resolution" | "rule" | "topic" | "question" | "domain" | "boundary";
        TraceRequestInputRepositoryContext: {
            /** @default null */
            freshness?: components["schemas"]["TraceRequestInputFreshnessPolicy"] | null;
            repository: string;
            scope: string;
        };
        /** @description Walk outward from a record for a bounded number of hops. */
        TraceRequestInputTraceQuery: {
            /** @default both */
            direction?: components["schemas"]["TraceRequestInputDirection"];
            id: string;
            /** @default false */
            include_retired?: boolean;
            /**
             * Format: uint
             * @default 50
             */
            limit?: number;
            /**
             * Format: uint
             * @default 3
             */
            max_depth?: number;
            /** @default null */
            node_type?: components["schemas"]["TraceRequestInputNodeType"] | null;
            /**
             * Format: uint32
             * @default null
             */
            protocol_version?: number | null;
            /** @default [] */
            relations?: string[];
        };
        /**
         * QueryResponse
         * @description The envelope every query primitive answers in.
         *
         *     The protocol version travels with the answer, so a caller holding a
         *     recorded response can tell which contract produced it, `operation`
         *     names which primitive it came from, and `stamp` says what the answer
         *     reflects.
         */
        TraceSuccessOutput: {
            freshness_cause?: components["schemas"]["TraceSuccessOutputFreshnessCause"] | null;
            freshness_error?: string | null;
            has_more: boolean;
            id: string;
            /** Format: uint */
            limit: number;
            /** Format: uint */
            max_depth: number;
            nodes: components["schemas"]["TraceSuccessOutputTracedNode"][];
            /** @constant */
            operation: "trace";
            /** @constant */
            protocol_version: 7;
            stamp: components["schemas"]["TraceSuccessOutputStamp"];
        };
        TraceSuccessOutputArtifactLink: {
            target_id: components["schemas"]["TraceSuccessOutputStableId"];
            target_type: components["schemas"]["TraceSuccessOutputArtifactLinkTargetType"];
        };
        /** @enum {string} */
        TraceSuccessOutputArtifactLinkTargetType: "source" | "requirement" | "resolution" | "rule";
        TraceSuccessOutputBoundary: {
            id: components["schemas"]["TraceSuccessOutputStableId"];
            requirement_id: components["schemas"]["TraceSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["TraceSuccessOutputScopeId"];
            source_ref?: components["schemas"]["TraceSuccessOutputSourceReference"] | null;
            statement: string;
        };
        /** @description One owner-local path to a language-authored declaration. */
        TraceSuccessOutputDeclarationAddress: string[];
        TraceSuccessOutputDomain: {
            color?: string | null;
            description?: string | null;
            id: components["schemas"]["TraceSuccessOutputStableId"];
            name: string;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["TraceSuccessOutputScopeId"];
        };
        /**
         * @description The stage that failed is known even when its lower-level error is not public.
         * @enum {string}
         */
        TraceSuccessOutputFreshnessCause: "catch_up_failed";
        /**
         * @description One canonical record as a query hands it back.
         *
         *     Each variant carries the record the store already writes, so a primitive
         *     never invents a second vocabulary for a Requirement or a Rule. The
         *     `node_type` tag is the same word a relation row uses for its endpoints.
         */
        TraceSuccessOutputGraphNode: components["schemas"]["TraceSuccessOutputSource"] | components["schemas"]["TraceSuccessOutputRequirement"] | components["schemas"]["TraceSuccessOutputResolution"] | components["schemas"]["TraceSuccessOutputRule"] | components["schemas"]["TraceSuccessOutputTopic"] | components["schemas"]["TraceSuccessOutputQuestion"] | components["schemas"]["TraceSuccessOutputDomain"] | components["schemas"]["TraceSuccessOutputBoundary"];
        TraceSuccessOutputQuestion: {
            answer?: string | null;
            /** Format: int64 */
            claimed_at?: number | null;
            claimed_by?: string | null;
            contradicts?: components["schemas"]["TraceSuccessOutputStableId"] | null;
            id: components["schemas"]["TraceSuccessOutputStableId"];
            /** @default [] */
            links: components["schemas"]["TraceSuccessOutputArtifactLink"][];
            question: string;
            requirement_id: components["schemas"]["TraceSuccessOutputStableId"];
            resolution_id?: components["schemas"]["TraceSuccessOutputStableId"] | null;
            /** @description The verb that resolves this question, chosen when the question is minted. */
            resolution_method: components["schemas"]["TraceSuccessOutputResolutionMethod"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["TraceSuccessOutputScopeId"];
            status: components["schemas"]["TraceSuccessOutputQuestionStatus"];
            topic_id: components["schemas"]["TraceSuccessOutputStableId"];
        };
        /** @enum {string} */
        TraceSuccessOutputQuestionStatus: "open" | "blocked_on_human" | "answered";
        TraceSuccessOutputRequirement: {
            declaration_address?: components["schemas"]["TraceSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            depends_on?: components["schemas"]["TraceSuccessOutputStableId"][];
            description?: string | null;
            domain_id?: components["schemas"]["TraceSuccessOutputStableId"] | null;
            /**
             * @description Deliberately unstructured free text: the dim view of decisions and
             *     investigations that are coming but cannot yet be phrased sharply.
             */
            fog?: string | null;
            id: components["schemas"]["TraceSuccessOutputStableId"];
            origin_message?: components["schemas"]["TraceSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["TraceSuccessOutputStableId"] | null;
            refines?: components["schemas"]["TraceSuccessOutputStableId"] | null;
            retired?: boolean;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["TraceSuccessOutputScopeId"];
            source_refs?: components["schemas"]["TraceSuccessOutputSourceReference"][];
            spawned_by?: components["schemas"]["TraceSuccessOutputStableId"] | null;
            statement: string;
            status: components["schemas"]["TraceSuccessOutputRequirementStatus"];
            supersedes?: components["schemas"]["TraceSuccessOutputStableId"][];
        };
        /** @enum {string} */
        TraceSuccessOutputRequirementStatus: "active" | "discovery" | "refinement" | "resolved";
        TraceSuccessOutputResolution: {
            /** Format: int64 */
            approved_at?: number | null;
            approved_by?: string | null;
            /** Format: double */
            confidence?: number | null;
            context?: string | null;
            enforcement?: string | null;
            id: components["schemas"]["TraceSuccessOutputStableId"];
            /** @default [] */
            inputs: components["schemas"]["TraceSuccessOutputResolutionInput"][];
            made_by?: string | null;
            origin_message?: components["schemas"]["TraceSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["TraceSuccessOutputStableId"] | null;
            position: string;
            rationale: string;
            requirement_ids?: components["schemas"]["TraceSuccessOutputStableId"][];
            review_on: string | null;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["TraceSuccessOutputScopeId"];
            status: components["schemas"]["TraceSuccessOutputResolutionStatus"];
            supersedes?: components["schemas"]["TraceSuccessOutputStableId"][];
            title: string;
        };
        TraceSuccessOutputResolutionInput: {
            input_type: components["schemas"]["TraceSuccessOutputResolutionInputType"];
            reference: string;
            summary: string;
        };
        /** @enum {string} */
        TraceSuccessOutputResolutionInputType: "regulatory" | "legal_advice" | "commercial" | "benchmark" | "technical" | "incident" | "source_material";
        /** @enum {string} */
        TraceSuccessOutputResolutionMethod: "grill" | "prototype" | "research" | "verify" | "task";
        /** @enum {string} */
        TraceSuccessOutputResolutionStatus: "draft" | "review" | "proposed" | "approved" | "rejected" | "revised" | "superseded" | "abandoned";
        TraceSuccessOutputRule: {
            declaration_address?: components["schemas"]["TraceSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            description?: string | null;
            id: components["schemas"]["TraceSuccessOutputStableId"];
            name?: string | null;
            origin_message?: components["schemas"]["TraceSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["TraceSuccessOutputStableId"] | null;
            requirement_ids?: components["schemas"]["TraceSuccessOutputStableId"][];
            resolution_ids?: components["schemas"]["TraceSuccessOutputStableId"][];
            retired?: boolean;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["TraceSuccessOutputScopeId"];
            severity: components["schemas"]["TraceSuccessOutputRuleSeverity"];
            source_document?: string | null;
            source_section?: string | null;
            statement: string;
            status: components["schemas"]["TraceSuccessOutputRuleStatus"];
        };
        /** @enum {string} */
        TraceSuccessOutputRuleSeverity: "low" | "medium" | "high" | "critical";
        /** @enum {string} */
        TraceSuccessOutputRuleStatus: "draft" | "review" | "active" | "deprecated" | "archived";
        /**
         * @description A scope id. The inner `String` is private and `new` is the only way in, so
         *     every `ScopeId` in existence satisfies [`is_well_formed_id`].
         */
        TraceSuccessOutputScopeId: string;
        TraceSuccessOutputSource: {
            commit_pin?: string | null;
            declaration_address?: components["schemas"]["TraceSuccessOutputDeclarationAddress"] | null;
            declared_by?: string | null;
            /** Format: int64 */
            effective_date?: number | null;
            id: components["schemas"]["TraceSuccessOutputStableId"];
            name: string;
            origin_message?: components["schemas"]["TraceSuccessOutputStableId"] | null;
            origin_thread?: components["schemas"]["TraceSuccessOutputStableId"] | null;
            reference?: string | null;
            retired?: boolean;
            /** Format: int64 */
            review_date?: number | null;
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["TraceSuccessOutputScopeId"];
            source_type: components["schemas"]["TraceSuccessOutputSourceType"];
            supersedes?: components["schemas"]["TraceSuccessOutputStableId"][];
            url: string | null;
        };
        TraceSuccessOutputSourceReference: {
            clause?: string | null;
            source_id: components["schemas"]["TraceSuccessOutputStableId"];
        };
        /** @enum {string} */
        TraceSuccessOutputSourceType: "policy" | "document" | "legislation" | "company_agreement" | "system_state" | "external_integration" | "domain_knowledge" | "project_artifact" | "incident" | "api_spec";
        /**
         * @description A stable artifact id. The inner `String` is private and `new` is the only
         *     way in, so every `StableId` in existence satisfies [`is_well_formed_id`].
         */
        TraceSuccessOutputStableId: string;
        /**
         * @description What a query answer reflects: the projection revision the rows came
         *     from, the freshness step the reader ran, and which parts of the answer
         *     the revision covers.
         *
         *     `serial` and `digest` name the latest `projection_revision` row and
         *     `instance_id` the `projection_instance` row; serials compare only within
         *     one instance. `attested` names the projection tables behind the answer.
         *     `live` names what the stamp does not cover, from a closed list:
         *     `canonical` (canonical shards), `scanned_sites` (a working-tree scan),
         *     `verification_runs` (cache JSONL), and `diff` (git). A stamp never
         *     implies freshness for anything it does not list.
         */
        TraceSuccessOutputStamp: {
            attested: string[];
            /**
             * Format: uint32
             * @description The reader logic version. It moves when the reader answers
             *     differently over the same rows, never for a migration.
             */
            derivation: number;
            digest: string;
            instance_id: string;
            live: string[];
            policy: components["schemas"]["TraceSuccessOutputStampPolicy"];
            /** Format: int64 */
            serial: number;
        };
        /** @description The freshness step a read ran before it answered. */
        TraceSuccessOutputStampPolicy: "catch_up" | "annotate_only" | "refuse_stale" | "catch_up_failed";
        TraceSuccessOutputTopic: {
            /** Format: int64 */
            claimed_at?: number | null;
            claimed_by?: string | null;
            id: components["schemas"]["TraceSuccessOutputStableId"];
            /** @default [] */
            links: components["schemas"]["TraceSuccessOutputArtifactLink"][];
            requirement_id: components["schemas"]["TraceSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["TraceSuccessOutputScopeId"];
            status: components["schemas"]["TraceSuccessOutputTopicStatus"];
            title: string;
        };
        /** @enum {string} */
        TraceSuccessOutputTopicStatus: "open" | "explored" | "closed";
        /** @description One record reached by a walk, with how many hops it took. */
        TraceSuccessOutputTracedNode: {
            /** Format: uint */
            depth: number;
            node: components["schemas"]["TraceSuccessOutputGraphNode"];
        };
        /** FailureEnvelope */
        VerificationBindingsFailureOutput: {
            error: components["schemas"]["VerificationBindingsFailureOutputOperationError"];
            /** @constant */
            operation?: "verification-bindings";
            /** @constant */
            protocol_version: 7;
        };
        /** @enum {string} */
        VerificationBindingsFailureOutputInvalidInputReason: "required" | "invalid_value" | "malformed_json" | "unknown_field" | "too_large";
        VerificationBindingsFailureOutputMovedUnit: {
            live: string;
            stored: string;
            unit: string;
        };
        /** @description Keeps the handler's native error separate from preparation failure. */
        VerificationBindingsFailureOutputOperationError: components["schemas"]["VerificationBindingsFailureOutputOperationFailure"] | components["schemas"]["VerificationBindingsFailureOutputReadFailure"];
        VerificationBindingsFailureOutputOperationFailure: {
            field: string | null;
            /** @constant */
            kind: "invalid_input";
            reason: components["schemas"]["VerificationBindingsFailureOutputInvalidInputReason"];
        } | {
            /** @constant */
            kind: "protocol_mismatch";
            /** Format: uint32 */
            requested: number;
            /** Format: uint32 */
            supported: number;
        } | {
            /** @constant */
            kind: "unknown_operation";
        } | {
            /** @constant */
            kind: "unauthenticated";
        } | {
            /** @constant */
            kind: "access_denied";
        } | {
            /** @constant */
            kind: "unknown_target";
        } | {
            /** @constant */
            kind: "unknown_scope";
        } | {
            /** @constant */
            kind: "unavailable_needs";
        } | {
            /** @constant */
            kind: "internal";
        };
        VerificationBindingsFailureOutputReadFailure: {
            /** @constant */
            kind: "file_access_denied";
        } | {
            /** @constant */
            kind: "file_unavailable";
        } | {
            /** @constant */
            kind: "git_unavailable";
        } | {
            /** @constant */
            kind: "git_revision_not_found";
        } | {
            /** @constant */
            kind: "no_projection";
        } | {
            digest: string;
            instance_id: string;
            /** @constant */
            kind: "stale";
            moved: components["schemas"]["VerificationBindingsFailureOutputMovedUnit"][];
            /** Format: int64 */
            serial: number;
        } | {
            /** @constant */
            kind: "unit_unreadable";
            unit: string;
        } | {
            /** @constant */
            kind: "schema_behind";
        } | {
            /** @constant */
            kind: "half_migrated";
        } | {
            /** @constant */
            kind: "read_failed";
        };
        /** RepositoryCall */
        VerificationBindingsRequestInput: {
            context: components["schemas"]["VerificationBindingsRequestInputRepositoryScope"];
            request: components["schemas"]["VerificationBindingsRequestInputVerificationListRequest"];
        };
        /** @description Scope selection for operations that do not use projection freshness. */
        VerificationBindingsRequestInputRepositoryScope: {
            repository: string;
            scope: string;
        };
        /**
         * @description A stable artifact id. The inner `String` is private and `new` is the only
         *     way in, so every `StableId` in existence satisfies [`is_well_formed_id`].
         */
        VerificationBindingsRequestInputStableId: string;
        /** @description Existing verification lists remain complete and have no page controls. */
        VerificationBindingsRequestInputVerificationListRequest: {
            /** @default null */
            rule?: components["schemas"]["VerificationBindingsRequestInputStableId"] | null;
        };
        /** Array_of_VerificationBinding */
        VerificationBindingsSuccessOutput: components["schemas"]["VerificationBindingsSuccessOutputVerificationBinding"][];
        /**
         * @description A scope id. The inner `String` is private and `new` is the only way in, so
         *     every `ScopeId` in existence satisfies [`is_well_formed_id`].
         */
        VerificationBindingsSuccessOutputScopeId: string;
        /**
         * @description A stable artifact id. The inner `String` is private and `new` is the only
         *     way in, so every `StableId` in existence satisfies [`is_well_formed_id`].
         */
        VerificationBindingsSuccessOutputStableId: string;
        /** @description One durable language-authored relationship from a code site to a Rule. */
        VerificationBindingsSuccessOutputVerificationBinding: {
            declared_by: string;
            file: string;
            id: components["schemas"]["VerificationBindingsSuccessOutputStableId"];
            key: string;
            method: components["schemas"]["VerificationBindingsSuccessOutputVerificationMethod"];
            retired?: boolean;
            rule_id: components["schemas"]["VerificationBindingsSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["VerificationBindingsSuccessOutputScopeId"];
            symbol?: string | null;
        };
        /**
         * @description How a verification binding supports its Rule.
         * @enum {string}
         */
        VerificationBindingsSuccessOutputVerificationMethod: "exhaustion" | "property" | "examples" | "conformance" | "construction" | "proof";
        /** FailureEnvelope */
        VerificationRunsFailureOutput: {
            error: components["schemas"]["VerificationRunsFailureOutputOperationError"];
            /** @constant */
            operation?: "verification-runs";
            /** @constant */
            protocol_version: 7;
        };
        /** @enum {string} */
        VerificationRunsFailureOutputInvalidInputReason: "required" | "invalid_value" | "malformed_json" | "unknown_field" | "too_large";
        VerificationRunsFailureOutputMovedUnit: {
            live: string;
            stored: string;
            unit: string;
        };
        /** @description Keeps the handler's native error separate from preparation failure. */
        VerificationRunsFailureOutputOperationError: components["schemas"]["VerificationRunsFailureOutputOperationFailure"] | components["schemas"]["VerificationRunsFailureOutputReadFailure"];
        VerificationRunsFailureOutputOperationFailure: {
            field: string | null;
            /** @constant */
            kind: "invalid_input";
            reason: components["schemas"]["VerificationRunsFailureOutputInvalidInputReason"];
        } | {
            /** @constant */
            kind: "protocol_mismatch";
            /** Format: uint32 */
            requested: number;
            /** Format: uint32 */
            supported: number;
        } | {
            /** @constant */
            kind: "unknown_operation";
        } | {
            /** @constant */
            kind: "unauthenticated";
        } | {
            /** @constant */
            kind: "access_denied";
        } | {
            /** @constant */
            kind: "unknown_target";
        } | {
            /** @constant */
            kind: "unknown_scope";
        } | {
            /** @constant */
            kind: "unavailable_needs";
        } | {
            /** @constant */
            kind: "internal";
        };
        VerificationRunsFailureOutputReadFailure: {
            /** @constant */
            kind: "file_access_denied";
        } | {
            /** @constant */
            kind: "file_unavailable";
        } | {
            /** @constant */
            kind: "git_unavailable";
        } | {
            /** @constant */
            kind: "git_revision_not_found";
        } | {
            /** @constant */
            kind: "no_projection";
        } | {
            digest: string;
            instance_id: string;
            /** @constant */
            kind: "stale";
            moved: components["schemas"]["VerificationRunsFailureOutputMovedUnit"][];
            /** Format: int64 */
            serial: number;
        } | {
            /** @constant */
            kind: "unit_unreadable";
            unit: string;
        } | {
            /** @constant */
            kind: "schema_behind";
        } | {
            /** @constant */
            kind: "half_migrated";
        } | {
            /** @constant */
            kind: "read_failed";
        };
        /** RepositoryCall */
        VerificationRunsRequestInput: {
            context: components["schemas"]["VerificationRunsRequestInputRepositoryScope"];
            request: components["schemas"]["VerificationRunsRequestInputVerificationListRequest"];
        };
        /** @description Scope selection for operations that do not use projection freshness. */
        VerificationRunsRequestInputRepositoryScope: {
            repository: string;
            scope: string;
        };
        /**
         * @description A stable artifact id. The inner `String` is private and `new` is the only
         *     way in, so every `StableId` in existence satisfies [`is_well_formed_id`].
         */
        VerificationRunsRequestInputStableId: string;
        /** @description Existing verification lists remain complete and have no page controls. */
        VerificationRunsRequestInputVerificationListRequest: {
            /** @default null */
            rule?: components["schemas"]["VerificationRunsRequestInputStableId"] | null;
        };
        /** Array_of_VerificationRun */
        VerificationRunsSuccessOutput: components["schemas"]["VerificationRunsSuccessOutputVerificationRun"][];
        /**
         * @description A scope id. The inner `String` is private and `new` is the only way in, so
         *     every `ScopeId` in existence satisfies [`is_well_formed_id`].
         */
        VerificationRunsSuccessOutputScopeId: string;
        /**
         * @description A stable artifact id. The inner `String` is private and `new` is the only
         *     way in, so every `StableId` in existence satisfies [`is_well_formed_id`].
         */
        VerificationRunsSuccessOutputStableId: string;
        /**
         * @description Volatile evidence from one language-owned verification callback.
         *
         *     Runs live in Provenance's derived cache rather than canonical state: a
         *     local test run must not dirty the repository. `rule_id` is the join back
         *     to the canonical graph.
         */
        VerificationRunsSuccessOutputVerificationRun: {
            binding_id?: components["schemas"]["VerificationRunsSuccessOutputStableId"] | null;
            commit?: string | null;
            /** Format: int64 */
            completed_at?: number | null;
            declared_by: string;
            error?: string | null;
            file?: string | null;
            id: components["schemas"]["VerificationRunsSuccessOutputStableId"];
            method: string;
            rule_id: components["schemas"]["VerificationRunsSuccessOutputStableId"];
            /** Format: uint32 */
            schema_version: number;
            scope_id: components["schemas"]["VerificationRunsSuccessOutputScopeId"];
            /** Format: int64 */
            started_at: number;
            status: components["schemas"]["VerificationRunsSuccessOutputVerificationRunStatus"];
            symbol?: string | null;
        };
        /**
         * @description The lifecycle state of one callback-backed verification run.
         * @enum {string}
         */
        VerificationRunsSuccessOutputVerificationRunStatus: "running" | "passed" | "failed";
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    metadata: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Engine metadata */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MetadataOutput"];
                };
            };
        };
    };
    checkStatement: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CheckStatementRequestInput"];
            };
        };
        responses: {
            /** @description Operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckStatementSuccessOutput"];
                };
            };
            /** @description Operation refused */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckStatementFailureOutput"];
                };
            };
            /** @description Operation refused */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckStatementFailureOutput"];
                };
            };
            /** @description Operation refused */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckStatementFailureOutput"];
                };
            };
            /** @description Operation refused */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckStatementFailureOutput"];
                };
            };
            /** @description Operation refused */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckStatementFailureOutput"];
                };
            };
            /** @description Operation refused */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckStatementFailureOutput"];
                };
            };
        };
    };
    evidence: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["EvidenceRequestInput"];
            };
        };
        responses: {
            /** @description Operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EvidenceSuccessOutput"];
                };
            };
            /** @description Operation refused */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EvidenceFailureOutput"];
                };
            };
            /** @description Operation refused */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EvidenceFailureOutput"];
                };
            };
            /** @description Operation refused */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EvidenceFailureOutput"];
                };
            };
            /** @description Operation refused */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EvidenceFailureOutput"];
                };
            };
            /** @description Operation refused */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EvidenceFailureOutput"];
                };
            };
            /** @description Operation refused */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EvidenceFailureOutput"];
                };
            };
            /** @description Operation refused */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EvidenceFailureOutput"];
                };
            };
        };
    };
    get: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["GetRequestInput"];
            };
        };
        responses: {
            /** @description Operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["GetSuccessOutput"];
                };
            };
            /** @description Operation refused */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["GetFailureOutput"];
                };
            };
            /** @description Operation refused */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["GetFailureOutput"];
                };
            };
            /** @description Operation refused */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["GetFailureOutput"];
                };
            };
            /** @description Operation refused */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["GetFailureOutput"];
                };
            };
            /** @description Operation refused */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["GetFailureOutput"];
                };
            };
            /** @description Operation refused */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["GetFailureOutput"];
                };
            };
            /** @description Operation refused */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["GetFailureOutput"];
                };
            };
        };
    };
    impact: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ImpactRequestInput"];
            };
        };
        responses: {
            /** @description Operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImpactSuccessOutput"];
                };
            };
            /** @description Operation refused */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImpactFailureOutput"];
                };
            };
            /** @description Operation refused */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImpactFailureOutput"];
                };
            };
            /** @description Operation refused */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImpactFailureOutput"];
                };
            };
            /** @description Operation refused */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImpactFailureOutput"];
                };
            };
            /** @description Operation refused */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImpactFailureOutput"];
                };
            };
            /** @description Operation refused */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImpactFailureOutput"];
                };
            };
            /** @description Operation refused */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImpactFailureOutput"];
                };
            };
        };
    };
    info: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["InfoRequestInput"];
            };
        };
        responses: {
            /** @description Operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InfoSuccessOutput"];
                };
            };
            /** @description Operation refused */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InfoFailureOutput"];
                };
            };
            /** @description Operation refused */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InfoFailureOutput"];
                };
            };
            /** @description Operation refused */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InfoFailureOutput"];
                };
            };
            /** @description Operation refused */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InfoFailureOutput"];
                };
            };
            /** @description Operation refused */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InfoFailureOutput"];
                };
            };
            /** @description Operation refused */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InfoFailureOutput"];
                };
            };
        };
    };
    neighbors: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["NeighborsRequestInput"];
            };
        };
        responses: {
            /** @description Operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NeighborsSuccessOutput"];
                };
            };
            /** @description Operation refused */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NeighborsFailureOutput"];
                };
            };
            /** @description Operation refused */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NeighborsFailureOutput"];
                };
            };
            /** @description Operation refused */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NeighborsFailureOutput"];
                };
            };
            /** @description Operation refused */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NeighborsFailureOutput"];
                };
            };
            /** @description Operation refused */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NeighborsFailureOutput"];
                };
            };
            /** @description Operation refused */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NeighborsFailureOutput"];
                };
            };
            /** @description Operation refused */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NeighborsFailureOutput"];
                };
            };
        };
    };
    resolveSymbol: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ResolveSymbolRequestInput"];
            };
        };
        responses: {
            /** @description Operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResolveSymbolSuccessOutput"];
                };
            };
            /** @description Operation refused */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResolveSymbolFailureOutput"];
                };
            };
            /** @description Operation refused */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResolveSymbolFailureOutput"];
                };
            };
            /** @description Operation refused */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResolveSymbolFailureOutput"];
                };
            };
            /** @description Operation refused */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResolveSymbolFailureOutput"];
                };
            };
            /** @description Operation refused */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResolveSymbolFailureOutput"];
                };
            };
            /** @description Operation refused */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResolveSymbolFailureOutput"];
                };
            };
            /** @description Operation refused */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResolveSymbolFailureOutput"];
                };
            };
        };
    };
    search: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SearchRequestInput"];
            };
        };
        responses: {
            /** @description Operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SearchSuccessOutput"];
                };
            };
            /** @description Operation refused */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SearchFailureOutput"];
                };
            };
            /** @description Operation refused */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SearchFailureOutput"];
                };
            };
            /** @description Operation refused */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SearchFailureOutput"];
                };
            };
            /** @description Operation refused */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SearchFailureOutput"];
                };
            };
            /** @description Operation refused */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SearchFailureOutput"];
                };
            };
            /** @description Operation refused */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SearchFailureOutput"];
                };
            };
            /** @description Operation refused */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SearchFailureOutput"];
                };
            };
        };
    };
    stale: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["StaleRequestInput"];
            };
        };
        responses: {
            /** @description Operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["StaleSuccessOutput"];
                };
            };
            /** @description Operation refused */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["StaleFailureOutput"];
                };
            };
            /** @description Operation refused */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["StaleFailureOutput"];
                };
            };
            /** @description Operation refused */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["StaleFailureOutput"];
                };
            };
            /** @description Operation refused */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["StaleFailureOutput"];
                };
            };
            /** @description Operation refused */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["StaleFailureOutput"];
                };
            };
            /** @description Operation refused */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["StaleFailureOutput"];
                };
            };
            /** @description Operation refused */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["StaleFailureOutput"];
                };
            };
        };
    };
    trace: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["TraceRequestInput"];
            };
        };
        responses: {
            /** @description Operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TraceSuccessOutput"];
                };
            };
            /** @description Operation refused */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TraceFailureOutput"];
                };
            };
            /** @description Operation refused */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TraceFailureOutput"];
                };
            };
            /** @description Operation refused */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TraceFailureOutput"];
                };
            };
            /** @description Operation refused */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TraceFailureOutput"];
                };
            };
            /** @description Operation refused */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TraceFailureOutput"];
                };
            };
            /** @description Operation refused */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TraceFailureOutput"];
                };
            };
            /** @description Operation refused */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TraceFailureOutput"];
                };
            };
        };
    };
    verificationBindings: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["VerificationBindingsRequestInput"];
            };
        };
        responses: {
            /** @description Operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationBindingsSuccessOutput"];
                };
            };
            /** @description Operation refused */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationBindingsFailureOutput"];
                };
            };
            /** @description Operation refused */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationBindingsFailureOutput"];
                };
            };
            /** @description Operation refused */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationBindingsFailureOutput"];
                };
            };
            /** @description Operation refused */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationBindingsFailureOutput"];
                };
            };
            /** @description Operation refused */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationBindingsFailureOutput"];
                };
            };
            /** @description Operation refused */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationBindingsFailureOutput"];
                };
            };
        };
    };
    verificationRuns: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["VerificationRunsRequestInput"];
            };
        };
        responses: {
            /** @description Operation result */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationRunsSuccessOutput"];
                };
            };
            /** @description Operation refused */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationRunsFailureOutput"];
                };
            };
            /** @description Operation refused */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationRunsFailureOutput"];
                };
            };
            /** @description Operation refused */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationRunsFailureOutput"];
                };
            };
            /** @description Operation refused */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationRunsFailureOutput"];
                };
            };
            /** @description Operation refused */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationRunsFailureOutput"];
                };
            };
            /** @description Operation refused */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VerificationRunsFailureOutput"];
                };
            };
        };
    };
}
