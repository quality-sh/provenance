// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn verification_runs(
        &self,
        call: &VerificationRunsRequestInput,
    ) -> Result<VerificationRunsSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/verification-runs", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: VerificationRunsFailureOutput =
                response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::VerificationRuns(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
}
