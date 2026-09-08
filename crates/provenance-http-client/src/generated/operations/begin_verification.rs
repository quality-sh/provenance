// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn begin_verification(
        &self,
        call: &BeginVerificationRequestInput,
    ) -> Result<BeginVerificationSuccessOutput, Error> {
        let response = self
            .http
            .post(format!(
                "{}/v7/operations/begin-verification",
                self.base_url
            ))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("begin_verification", true, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "begin_verification", true).await?;
        if !status.is_success() {
            runtime::validate(
                &value,
                "BeginVerificationFailureOutput",
                "begin_verification",
                true,
            )?;
            let uncertain = runtime::uncertain_kind(&value, true);
            let failure: BeginVerificationFailureOutput =
                runtime::decode(value, "begin_verification", true)?;
            let failure = OperationFailure::BeginVerification(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("begin_verification", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(
            &value,
            "BeginVerificationSuccessOutput",
            "begin_verification",
            true,
        )?;
        runtime::decode(value, "begin_verification", true)
    }
}
