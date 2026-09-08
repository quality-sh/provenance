// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn create_source(
        &self,
        call: &CreateSourceRequestInput,
    ) -> Result<CreateSourceSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/create-source", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("create_source", true, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "create_source", true).await?;
        if !status.is_success() {
            runtime::validate(&value, "CreateSourceFailureOutput", "create_source", true)?;
            let uncertain = runtime::uncertain_kind(&value, true);
            let failure: CreateSourceFailureOutput = runtime::decode(value, "create_source", true)?;
            let failure = OperationFailure::CreateSource(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("create_source", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(&value, "CreateSourceSuccessOutput", "create_source", true)?;
        runtime::decode(value, "create_source", true)
    }
}
