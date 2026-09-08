// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn trace(&self, call: &TraceRequestInput) -> Result<TraceSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/trace", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: TraceFailureOutput = response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::Trace(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
}
