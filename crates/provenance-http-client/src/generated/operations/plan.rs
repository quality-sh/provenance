// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn plan(&self, call: &PlanRequestInput) -> Result<PlanSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/plan", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("plan", false, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "plan", false).await?;
        if !status.is_success() {
            runtime::validate(&value, "PlanFailureOutput", "plan", false)?;
            let uncertain = runtime::uncertain_kind(&value, false);
            let failure: PlanFailureOutput = runtime::decode(value, "plan", false)?;
            let failure = OperationFailure::Plan(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("plan", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(&value, "PlanSuccessOutput", "plan", false)?;
        runtime::decode(value, "plan", false)
    }
}
