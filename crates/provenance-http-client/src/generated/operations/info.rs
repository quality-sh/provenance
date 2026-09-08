// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn info(&self, call: &InfoRequestInput) -> Result<InfoSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/info", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: InfoFailureOutput = response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::Info(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
}
