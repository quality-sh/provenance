// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn complete_verification(
        &self,
        call: &CompleteVerificationRequestInput,
    ) -> Result<CompleteVerificationSuccessOutput, Error> {
        let response = self
            .http
            .post(format!(
                "{}/v7/operations/complete-verification",
                self.base_url
            ))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("complete_verification", true, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "complete_verification", true).await?;
        if !status.is_success() {
            runtime::validate(
                &value,
                "CompleteVerificationFailureOutput",
                "complete_verification",
                true,
            )?;
            let uncertain = runtime::uncertain_kind(&value, true);
            let failure: CompleteVerificationFailureOutput =
                runtime::decode(value, "complete_verification", true)?;
            let failure = OperationFailure::CompleteVerification(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("complete_verification", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(
            &value,
            "CompleteVerificationSuccessOutput",
            "complete_verification",
            true,
        )?;
        runtime::decode(value, "complete_verification", true)
    }
}
