// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn apply(&self, call: &ApplyRequestInput) -> Result<ApplySuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/apply", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("apply", true, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "apply", true).await?;
        if !status.is_success() {
            runtime::validate(&value, "ApplyFailureOutput", "apply", true)?;
            let uncertain = runtime::uncertain_kind(&value, true);
            let failure: ApplyFailureOutput = runtime::decode(value, "apply", true)?;
            let failure = OperationFailure::Apply(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("apply", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(&value, "ApplySuccessOutput", "apply", true)?;
        runtime::decode(value, "apply", true)
    }
}
