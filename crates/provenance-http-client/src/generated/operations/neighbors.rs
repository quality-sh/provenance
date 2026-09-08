// Generated from OpenAPI. Do not edit.
impl HttpClient {
    pub async fn neighbors(
        &self,
        call: &NeighborsRequestInput,
    ) -> Result<NeighborsSuccessOutput, Error> {
        let response = self
            .http
            .post(format!("{}/v7/operations/neighbors", self.base_url))
            .json(call)
            .send()
            .await
            .map_err(|cause| runtime::connection("neighbors", false, cause))?;
        let status = response.status();
        let value = runtime::read_json(response, "neighbors", false).await?;
        if !status.is_success() {
            runtime::validate(&value, "NeighborsFailureOutput", "neighbors", false)?;
            let uncertain = runtime::uncertain_kind(&value, false);
            let failure: NeighborsFailureOutput = runtime::decode(value, "neighbors", false)?;
            let failure = OperationFailure::Neighbors(Box::new(failure));
            if uncertain {
                return Err(runtime::uncertain("neighbors", failure));
            }
            return Err(Error::Operation {
                status: status.as_u16(),
                failure,
            });
        }
        runtime::validate(&value, "NeighborsSuccessOutput", "neighbors", false)?;
        runtime::decode(value, "neighbors", false)
    }
}
