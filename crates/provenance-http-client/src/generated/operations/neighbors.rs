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
            .map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let failure: NeighborsFailureOutput =
                response.json().await.map_err(Error::Transport)?;
            return Err(Error::Operation {
                status: status.as_u16(),
                failure: OperationFailure::Neighbors(Box::new(failure)),
            });
        }
        response.json().await.map_err(Error::Transport)
    }
}
