// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn evidence(
        &self,
        call: &EvidenceRequestInput,
    ) -> Result<EvidenceSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/evidence", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("evidence", false, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "evidence", false).await?;
        if !status.is_success() {
            runtime::validate(&value, "EvidenceFailureOutput", "evidence", false)?;
            let uncertain = runtime::uncertain_kind(&value, false);
            let failure: EvidenceFailureOutput = runtime::decode(value, "evidence", false)?;
            let failure = OperationFailure::Evidence(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("evidence", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(&value, "EvidenceSuccessOutput", "evidence", false)?;
        runtime::decode(value, "evidence", false)
    }
}
