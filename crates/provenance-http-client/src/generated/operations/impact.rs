// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn impact(&self, call: &ImpactRequestInput) -> Result<ImpactSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/impact", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("impact", false, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "impact", false).await?;
        if !status.is_success() {
            runtime::validate(&value, "ImpactFailureOutput", "impact", false)?;
            let uncertain = runtime::uncertain_kind(&value, false);
            let failure: ImpactFailureOutput = runtime::decode(value, "impact", false)?;
            let failure = OperationFailure::Impact(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("impact", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(&value, "ImpactSuccessOutput", "impact", false)?;
        runtime::decode(value, "impact", false)
    }
}
