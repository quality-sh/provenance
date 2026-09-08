// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn search(&self, call: &SearchRequestInput) -> Result<SearchSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/search", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: SearchFailureOutput = response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::Search(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
}
