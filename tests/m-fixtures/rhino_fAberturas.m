let
    Fonte = Excel.Workbook(File.Contents("C:\Dados\Fluxo de Caixa Tab. Fato - Rhino.xlsx"), null, true),
    #"Aberturas do Mês_Sheet" = Fonte{[Item="Aberturas do Mês",Kind="Sheet"]}[Data],
    #"Cabeçalhos Promovidos" = Table.PromoteHeaders(#"Aberturas do Mês_Sheet", [PromoteAllScalars=true]),
    #"Tipo Alterado" = Table.TransformColumnTypes(#"Cabeçalhos Promovidos",{{"Data", type date}, {"Valor", Currency.Type}}),
    #"Erros Removidos" = Table.RemoveRowsWithErrors(#"Tipo Alterado"),
    #"Linhas em Branco Removidas" = Table.SelectRows(#"Erros Removidos", each not List.IsEmpty(List.RemoveMatchingItems(Record.FieldValues(_), {"", null})))
in
    #"Linhas em Branco Removidas"
