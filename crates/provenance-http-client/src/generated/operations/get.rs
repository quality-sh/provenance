// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn get(&self, call: &GetRequestInput) -> Result<GetSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/get", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("get", false, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "get", false).await?;
        if !status.is_success() {
            runtime::validate(&value, "GetFailureOutput", "get", false)?;
            let uncertain = runtime::uncertain_kind(&value, false);
            let failure: GetFailureOutput = runtime::decode(value, "get", false)?;
            let failure = OperationFailure::Get(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("get", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(&value, "GetSuccessOutput", "get", false)?;
        runtime::decode(value, "get", false)
    }
}
