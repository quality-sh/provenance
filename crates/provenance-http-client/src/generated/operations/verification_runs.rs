// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn verification_runs(
        &self,
        call: &VerificationRunsRequestInput,
    ) -> Result<VerificationRunsSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/verification-runs", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("verification_runs", false, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "verification_runs", false).await?;
        if !status.is_success() {
            runtime::validate(
                &value,
                "VerificationRunsFailureOutput",
                "verification_runs",
                false,
            )?;
            let uncertain = runtime::uncertain_kind(&value, false);
            let failure: VerificationRunsFailureOutput =
                runtime::decode(value, "verification_runs", false)?;
            let failure = OperationFailure::VerificationRuns(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("verification_runs", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(
            &value,
            "VerificationRunsSuccessOutput",
            "verification_runs",
            false,
        )?;
        runtime::decode(value, "verification_runs", false)
    }
}
