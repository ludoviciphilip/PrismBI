let
    Source = Excel.Workbook(File.Contents("C:\Dados\Fluxo de Caixa Tab. Fato - Rhino.xlsx"), null, true),
    Categorias_Sheet = Source{[Item="Categorias",Kind="Sheet"]}[Data],
    #"Promoted Headers" = Table.PromoteHeaders(Categorias_Sheet, [PromoteAllScalars=true]),
    #"Changed Type" = Table.TransformColumnTypes(#"Promoted Headers",{{"Categoria", type text}, {"Grupo DRE", type text}, {"Ordem DRE", Int64.Type}, {"Entra na DRE (competência)", type text}, {"Grupo Fluxo de Caixa", type text}, {"Entra no Fluxo de Caixa (caixa)", type text}, {"Observação", type text}}),
    #"Renamed Columns" = Table.RenameColumns(#"Changed Type",{{"Entra na DRE (competência)", "DRE"}, {"Entra no Fluxo de Caixa (caixa)", "DFC"}}),
    #"Erros Removidos" = Table.RemoveRowsWithErrors(#"Renamed Columns"),
    #"Erros Removidos1" = Table.RemoveRowsWithErrors(#"Erros Removidos"),
    #"Linhas em Branco Removidas" = Table.SelectRows(#"Erros Removidos1", each not List.IsEmpty(List.RemoveMatchingItems(Record.FieldValues(_), {"", null})))
in
    #"Linhas em Branco Removidas"
