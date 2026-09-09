export interface FixtureHost {
  environment: {
    PROVENANCE_ENDPOINT: string;
    PROVENANCE_TOKEN: string;
    PROVENANCE_REPOSITORY_ID: string;
    PROVENANCE_LOCAL_ROOT: string;
    PROVENANCE_SCOPE: string;
    PROVENANCE_BIN: undefined;
    PROVENANCE_REPO: undefined;
  };
  close(): Promise<void>;
}
export function startFixtureHost(options: {
  root: string;
  binary?: string;
  arguments?: string[];
  repositoryId?: string;
  scope?: string;
}): Promise<FixtureHost>;
