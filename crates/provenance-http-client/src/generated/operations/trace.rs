// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn trace(&self, call: &TraceRequestInput) -> Result<TraceSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/trace", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("trace", false, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "trace", false).await?;
        if !status.is_success() {
            runtime::validate(&value, "TraceFailureOutput", "trace", false)?;
            let uncertain = runtime::uncertain_kind(&value, false);
            let failure: TraceFailureOutput = runtime::decode(value, "trace", false)?;
            let failure = OperationFailure::Trace(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("trace", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(&value, "TraceSuccessOutput", "trace", false)?;
        runtime::decode(value, "trace", false)
    }
}
