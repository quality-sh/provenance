// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn add_source_reference(
        &self,
        call: &AddSourceReferenceRequestInput,
    ) -> Result<AddSourceReferenceSuccessOutput, Error> {
        let response = self
            .http
            .post(format!(
                "{}/v7/operations/add-source-reference",
                self.base_url
            ))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("add_source_reference", true, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "add_source_reference", true).await?;
        if !status.is_success() {
            runtime::validate(
                &value,
                "AddSourceReferenceFailureOutput",
                "add_source_reference",
                true,
            )?;
            let uncertain = runtime::uncertain_kind(&value, true);
            let failure: AddSourceReferenceFailureOutput =
                runtime::decode(value, "add_source_reference", true)?;
            let failure = OperationFailure::AddSourceReference(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("add_source_reference", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(
            &value,
            "AddSourceReferenceSuccessOutput",
            "add_source_reference",
            true,
        )?;
        runtime::decode(value, "add_source_reference", true)
    }
}
