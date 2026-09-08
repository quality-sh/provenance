// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn search(&self, call: &SearchRequestInput) -> Result<SearchSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/search", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("search", false, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "search", false).await?;
        if !status.is_success() {
            runtime::validate(&value, "SearchFailureOutput", "search", false)?;
            let uncertain = runtime::uncertain_kind(&value, false);
            let failure: SearchFailureOutput = runtime::decode(value, "search", false)?;
            let failure = OperationFailure::Search(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("search", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(&value, "SearchSuccessOutput", "search", false)?;
        runtime::decode(value, "search", false)
    }
}
