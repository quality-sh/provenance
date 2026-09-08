// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn info(&self, call: &InfoRequestInput) -> Result<InfoSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/info", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("info", false, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "info", false).await?;
        if !status.is_success() {
            runtime::validate(&value, "InfoFailureOutput", "info", false)?;
            let uncertain = runtime::uncertain_kind(&value, false);
            let failure: InfoFailureOutput = runtime::decode(value, "info", false)?;
            let failure = OperationFailure::Info(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("info", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(&value, "InfoSuccessOutput", "info", false)?;
        runtime::decode(value, "info", false)
    }
}
