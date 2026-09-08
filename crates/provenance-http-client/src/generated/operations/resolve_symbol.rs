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
            .map_err(|cause| runtime::connection("resolve_symbol", false, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "resolve_symbol", false).await?;
        if !status.is_success() {
            runtime::validate(
                &value,
                "ResolveSymbolFailureOutput",
                "resolve_symbol",
                false,
            )?;
            let uncertain = runtime::uncertain_kind(&value, false);
            let failure: ResolveSymbolFailureOutput =
                runtime::decode(value, "resolve_symbol", false)?;
            let failure = OperationFailure::ResolveSymbol(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("resolve_symbol", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(
            &value,
            "ResolveSymbolSuccessOutput",
            "resolve_symbol",
            false,
        )?;
        runtime::decode(value, "resolve_symbol", false)
    }
}
