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
        /** HostMetadata */
        MetadataOutput: {
            engine_version: string;
            /** @constant */
            protocol_version: 7;
        };
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
            /** @description Invalid request */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckStatementFailureOutput"];
                };
            };
            /** @description Authentication required */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckStatementFailureOutput"];
                };
            };
            /** @description Access denied */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckStatementFailureOutput"];
                };
            };
            /** @description Unknown operation or target */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckStatementFailureOutput"];
                };
            };
            /** @description Internal failure */
            500: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckStatementFailureOutput"];
                };
            };
            /** @description Execution resources unavailable */
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
}
