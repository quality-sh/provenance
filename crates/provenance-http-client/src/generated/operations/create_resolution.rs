// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn create_resolution(
        &self,
        call: &CreateResolutionRequestInput,
    ) -> Result<CreateResolutionSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/create-resolution", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("create_resolution", true, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "create_resolution", true).await?;
        if !status.is_success() {
            runtime::validate(
                &value,
                "CreateResolutionFailureOutput",
                "create_resolution",
                true,
            )?;
            let uncertain = runtime::uncertain_kind(&value, true);
            let failure: CreateResolutionFailureOutput =
                runtime::decode(value, "create_resolution", true)?;
            let failure = OperationFailure::CreateResolution(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("create_resolution", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(
            &value,
            "CreateResolutionSuccessOutput",
            "create_resolution",
            true,
        )?;
        runtime::decode(value, "create_resolution", true)
    }
}
