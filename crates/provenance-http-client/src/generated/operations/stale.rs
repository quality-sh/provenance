// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn stale(&self, call: &StaleRequestInput) -> Result<StaleSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/stale", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("stale", false, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "stale", false).await?;
        if !status.is_success() {
            runtime::validate(&value, "StaleFailureOutput", "stale", false)?;
            let uncertain = runtime::uncertain_kind(&value, false);
            let failure: StaleFailureOutput = runtime::decode(value, "stale", false)?;
            let failure = OperationFailure::Stale(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("stale", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(&value, "StaleSuccessOutput", "stale", false)?;
        runtime::decode(value, "stale", false)
    }
}
