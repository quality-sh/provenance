// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn verification_bindings(
        &self,
        call: &VerificationBindingsRequestInput,
    ) -> Result<VerificationBindingsSuccessOutput, Error> {
        let response = self
            .http
            .post(format!(
                "{}/v7/operations/verification-bindings",
                self.base_url
            ))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("verification_bindings", false, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "verification_bindings", false).await?;
        if !status.is_success() {
            runtime::validate(
                &value,
                "VerificationBindingsFailureOutput",
                "verification_bindings",
                false,
            )?;
            let uncertain = runtime::uncertain_kind(&value, false);
            let failure: VerificationBindingsFailureOutput =
                runtime::decode(value, "verification_bindings", false)?;
            let failure = OperationFailure::VerificationBindings(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("verification_bindings", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(
            &value,
            "VerificationBindingsSuccessOutput",
            "verification_bindings",
            false,
        )?;
        runtime::decode(value, "verification_bindings", false)
    }
}
