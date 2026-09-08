// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn resolve_symbol(
        &self,
        call: &ResolveSymbolRequestInput,
    ) -> Result<ResolveSymbolSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/resolve-symbol", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: ResolveSymbolFailureOutput =
                response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::ResolveSymbol(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
}
