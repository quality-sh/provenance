// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn create_requirement(
        &self,
        call: &CreateRequirementRequestInput,
    ) -> Result<CreateRequirementSuccessOutput, Error> {
        let response = self
            .http
            .post(format!(
                "{}/v7/operations/create-requirement",
                self.base_url
            ))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("create_requirement", true, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "create_requirement", true).await?;
        if !status.is_success() {
            runtime::validate(
                &value,
                "CreateRequirementFailureOutput",
                "create_requirement",
                true,
            )?;
            let uncertain = runtime::uncertain_kind(&value, true);
            let failure: CreateRequirementFailureOutput =
                runtime::decode(value, "create_requirement", true)?;
            let failure = OperationFailure::CreateRequirement(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("create_requirement", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(
            &value,
            "CreateRequirementSuccessOutput",
            "create_requirement",
            true,
        )?;
        runtime::decode(value, "create_requirement", true)
    }
}
