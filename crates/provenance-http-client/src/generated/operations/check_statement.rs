// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn check_statement(
        &self,
        call: &CheckStatementRequestInput,
    ) -> Result<CheckStatementSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/check-statement", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("check_statement", false, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "check_statement", false).await?;
        if !status.is_success() {
            runtime::validate(
                &value,
                "CheckStatementFailureOutput",
                "check_statement",
                false,
            )?;
            let uncertain = runtime::uncertain_kind(&value, false);
            let failure: CheckStatementFailureOutput =
                runtime::decode(value, "check_statement", false)?;
            let failure = OperationFailure::CheckStatement(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("check_statement", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(
            &value,
            "CheckStatementSuccessOutput",
            "check_statement",
            false,
        )?;
        runtime::decode(value, "check_statement", false)
    }
}
