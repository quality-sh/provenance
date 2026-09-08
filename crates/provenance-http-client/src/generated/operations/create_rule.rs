// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn create_rule(
        &self,
        call: &CreateRuleRequestInput,
    ) -> Result<CreateRuleSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/create-rule", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("create_rule", true, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "create_rule", true).await?;
        if !status.is_success() {
            runtime::validate(&value, "CreateRuleFailureOutput", "create_rule", true)?;
            let uncertain = runtime::uncertain_kind(&value, true);
            let failure: CreateRuleFailureOutput = runtime::decode(value, "create_rule", true)?;
            let failure = OperationFailure::CreateRule(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("create_rule", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(&value, "CreateRuleSuccessOutput", "create_rule", true)?;
        runtime::decode(value, "create_rule", true)
    }
}
