// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn verification_bindings(
        &self,
        call: &VerificationBindingsRequestInput,
    ) -> Result<VerificationBindingsSuccessOutput, Error> {
        let response = self
            .http
            .post(format!(
                "{}/v7/operations/verification-bindings",
                self.base_url
            ))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: VerificationBindingsFailureOutput =
                response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::VerificationBindings(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
}
