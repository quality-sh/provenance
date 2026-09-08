// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn check_statement(
        &self,
        call: &CheckStatementRequestInput,
    ) -> Result<CheckStatementSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/check-statement", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: CheckStatementFailureOutput =
                response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::CheckStatement(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
}
