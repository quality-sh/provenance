// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn stale(&self, call: &StaleRequestInput) -> Result<StaleSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/stale", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: StaleFailureOutput = response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::Stale(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
}
